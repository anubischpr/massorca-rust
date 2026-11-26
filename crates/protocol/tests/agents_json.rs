//! The agent wire (`docs/architecture/agents.md`): field names, omitted optionals, defaults and unknown enum values.

use std::collections::BTreeMap;
use std::fmt::Debug;

use massorca_protocol::methods::agents::AgentArchive;
use massorca_protocol::methods::agents::{
    self, AccountOrigin, AccountState, AccountSummary, Activity, ActivitySource, ActivityState,
    AgentBringBackParams, AgentDelegateParams, AgentHookParams, AgentKind, AgentKinds,
    AgentModeSet, AgentQueueEditParams, AgentQueueItemRef, AgentQueueList, AgentQueueParams,
    AgentResumeParams, AgentSession, AgentSetModeParams, AgentStartParams, AgentWatchEvent,
    Execution, KindCredentials, KindEffort, KindEffortLevel, KindModel, KindModelChoice,
    KindPackage, ModeApplied, ModeSource, Outcome, PackageState, PermissionMode, PromptReceipt,
    ReceiptKind, StatusMechanism,
};
use massorca_protocol::methods::agents::{
    AgentArchiveSessionParams, AgentForgetParams, AgentForgetPreview, AgentForgotten,
    AgentListParams, AgentModel, AgentModels, AgentModelsParams, ForgetPreviewWorkspace, ModeTone,
    ModelListingError, ModelListingFailure, ModelSource, WorkspaceRemovalError,
};
use massorca_protocol::methods::chat::RunType;
use massorca_protocol::methods::terminals::{ExitReason, TerminalExit, TerminalState};
use massorca_protocol::methods::workspaces::WorkspaceKind;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn live_session() -> AgentSession {
    AgentSession {
        id: "s-1".to_owned(),
        kind: "claude".to_owned(),
        workspace_id: "w-1".to_owned(),
        title: None,
        read_only: false,
        parent_id: None,
        brought_back_at_ms: None,
        agent_ref: None,
        terminal_id: Some("t-1".to_owned()),
        run: Default::default(),
        created_at_ms: 1_767_225_600_000,
        execution: Execution {
            state: TerminalState::Live,
            exit: None,
        },
        activity: None,
        result: None,
        holds_secrets: false,
        secrets_withheld: None,
        permission_mode: None,
        mode_confirmed: false,
        mode_source: None,
        pending_permission_mode: None,
        retained: false,
        released: false,
        archived: false,
        resume: None,
        delegation: None,
        fast: None,
        notes: Vec::new(),
        shared_folder_with: Vec::new(),
    }
}

#[test]
fn method_names_match_the_wire_table() {
    let names = [
        (agents::KINDS, "agent.kinds"),
        (agents::MODELS, "agent.models"),
        (agents::START, "agent.start"),
        (agents::LIST, "agent.list"),
        (agents::SHOW, "agent.show"),
        (agents::STOP, "agent.stop"),
        (agents::FORGET, "agent.forget"),
        (agents::PROMPT, "agent.prompt"),
        (agents::RESUME, "agent.resume"),
        (agents::MARK_SEEN, "agent.mark_seen"),
        (agents::HOOK, "agent.hook"),
        (agents::WATCH, "agent.watch"),
        (agents::DELEGATE, "agent.delegate"),
        (agents::TREE, "agent.tree"),
        (agents::BRING_BACK, "agent.bring_back"),
        (agents::CREDENTIALS_IMPORT, "agent.credentials.import"),
        (agents::CREDENTIALS_SHOW, "agent.credentials.show"),
        (agents::CREDENTIALS_REMOVE, "agent.credentials.remove"),
        (agents::ACCOUNTS_LIST, "agent.accounts.list"),
        (agents::ACCOUNTS_ADD, "agent.accounts.add"),
        (agents::ACCOUNTS_SIGN_IN, "agent.accounts.sign_in"),
        (agents::ACCOUNTS_IMPORT, "agent.accounts.import"),
        (
            agents::ACCOUNTS_IMPORT_CANDIDATES,
            "agent.accounts.import_candidates",
        ),
        (agents::ACCOUNTS_ACTIVATE, "agent.accounts.activate"),
        (agents::ACCOUNTS_REMOVE, "agent.accounts.remove"),
        (agents::ACCOUNTS_CHECK, "agent.accounts.check"),
    ];
    for (constant, expected) in names {
        assert_eq!(constant, expected);
    }
}

#[test]
fn a_live_session_omits_absent_fields() {
    assert_wire(
        &live_session(),
        &json!({
            "id": "s-1",
            "kind": "claude",
            "workspace_id": "w-1",
            "read_only": false,
            "terminal_id": "t-1",
            "created_at_ms": 1_767_225_600_000_i64,
            "execution": { "state": "live" }
        }),
    );
}

#[test]
fn a_chat_session_names_its_run_and_an_absent_run_reads_as_terminal() {
    let chat = AgentSession {
        terminal_id: None,
        run: RunType::Chat,
        ..live_session()
    };
    assert_wire(
        &chat,
        &json!({
            "id": "s-1",
            "kind": "claude",
            "workspace_id": "w-1",
            "read_only": false,
            "run": "chat",
            "created_at_ms": 1_767_225_600_000_i64,
            "execution": { "state": "live" }
        }),
    );
    let older: AgentSession = serde_json::from_value(json!({
        "id": "s-1",
        "kind": "claude",
        "workspace_id": "w-1",
        "read_only": false,
        "created_at_ms": 1,
        "execution": { "state": "live" }
    }))
    .expect("deserializes");
    assert_eq!(older.run, RunType::Terminal);
}

#[test]
fn an_exited_session_carries_its_exit_activity_and_result() {
    let session = AgentSession {
        title: Some("fix the build".to_owned()),
        read_only: true,
        parent_id: Some("s-0".to_owned()),
        brought_back_at_ms: Some(1_767_225_603_000),
        agent_ref: Some("ref-1".to_owned()),
        execution: Execution {
            state: TerminalState::Exited,
            exit: Some(TerminalExit {
                code: None,
                signal: None,
                reason: ExitReason::ServiceStopped,
                at_ms: 1_767_225_602_000,
            }),
        },
        activity: Some(Activity {
            state: ActivityState::Done,
            outcome: Some(Outcome::Interrupted),
            source: ActivitySource::Process,
            at_ms: 1_767_225_602_000,
            seen_at_ms: None,
        }),
        result: Some("all green".to_owned()),
        holds_secrets: false,
        secrets_withheld: None,
        permission_mode: None,
        mode_confirmed: false,
        mode_source: None,
        pending_permission_mode: None,
        retained: false,
        released: false,
        archived: false,
        resume: None,
        delegation: None,
        notes: Vec::new(),
        ..live_session()
    };
    assert_wire(
        &session,
        &json!({
            "id": "s-1",
            "kind": "claude",
            "workspace_id": "w-1",
            "title": "fix the build",
            "read_only": true,
            "parent_id": "s-0",
            "brought_back_at_ms": 1_767_225_603_000_i64,
            "agent_ref": "ref-1",
            "terminal_id": "t-1",
            "created_at_ms": 1_767_225_600_000_i64,
            "execution": {
                "state": "exited",
                "exit": { "reason": "service_stopped", "at_ms": 1_767_225_602_000_i64 }
            },
            "activity": {
                "state": "done",
                "outcome": "interrupted",
                "source": "process",
                "at_ms": 1_767_225_602_000_i64
            },
            "result": "all green"
        }),
    );
}

#[test]
fn a_kind_lists_its_mechanism_in_kebab_case_and_omits_no_problems() {
    let kind = AgentKind {
        id: "gentle-shell".to_owned(),
        name: "Gentle Shell".to_owned(),
        icon: "rose".to_owned(),
        owner: "core".to_owned(),
        installed: true,
        detected: true,
        program_path: Some("/usr/local/bin/gentle-shell".to_owned()),
        package: KindPackage {
            state: PackageState::Enabled,
            id: None,
            version: None,
        },
        resumable: false,
        status_mechanism: StatusMechanism::ClaudeSettings,
        credentials: KindCredentials {
            supported: false,
            imported: false,
        },
        problems: Vec::new(),
        permission_modes: Vec::new(),
        default_permission_mode: None,
        chat: None,
        model: None,
        fast: None,
        effort: None,
    };
    assert_wire(
        &AgentKinds { kinds: vec![kind] },
        &json!({ "kinds": [{
            "id": "gentle-shell",
            "name": "Gentle Shell",
            "icon": "rose",
            "owner": "core",
            "installed": true,
            "detected": true,
            "program_path": "/usr/local/bin/gentle-shell",
            "package": { "state": "enabled" },
            "resumable": false,
            "status_mechanism": "claude-settings",
            "credentials": { "supported": false, "imported": false }
        }]}),
    );
}

#[test]
fn a_kind_lists_its_model_choices_and_effort_levels_and_an_older_one_has_neither() {
    let kind = AgentKind {
        id: "claude".to_owned(),
        name: "Claude".to_owned(),
        icon: "claude".to_owned(),
        owner: "core".to_owned(),
        installed: true,
        detected: true,
        program_path: None,
        package: KindPackage {
            state: PackageState::Enabled,
            id: None,
            version: None,
        },
        resumable: true,
        status_mechanism: StatusMechanism::ClaudeSettings,
        credentials: KindCredentials {
            supported: true,
            imported: false,
        },
        problems: Vec::new(),
        permission_modes: Vec::new(),
        default_permission_mode: None,
        chat: None,
        model: Some(KindModel {
            choices: vec![
                KindModelChoice {
                    id: "sonnet".to_owned(),
                    label: "Sonnet".to_owned(),
                    default: true,
                    fast: false,
                    efforts: None,
                },
                KindModelChoice {
                    id: "haiku".to_owned(),
                    label: "Haiku".to_owned(),
                    default: false,
                    fast: false,
                    efforts: Some(Vec::new()),
                },
            ],
        }),
        fast: None,
        effort: Some(KindEffort {
            levels: vec![KindEffortLevel {
                id: "high".to_owned(),
                label: "High".to_owned(),
            }],
            default: Some("high".to_owned()),
        }),
    };
    assert_wire(
        &AgentKinds { kinds: vec![kind] },
        &json!({ "kinds": [{
            "id": "claude",
            "name": "Claude",
            "icon": "claude",
            "owner": "core",
            "installed": true,
            "detected": true,
            "package": { "state": "enabled" },
            "resumable": true,
            "status_mechanism": "claude-settings",
            "credentials": { "supported": true, "imported": false },
            "model": { "choices": [
                { "id": "sonnet", "label": "Sonnet", "default": true },
                { "id": "haiku", "label": "Haiku", "efforts": [] }
            ] },
            "effort": { "levels": [{ "id": "high", "label": "High" }], "default": "high" }
        }]}),
    );
    let older: AgentKinds = serde_json::from_value(json!({ "kinds": [{
        "id": "codex",
        "name": "Codex",
        "icon": "codex",
        "owner": "core",
        "installed": false,
        "resumable": true,
        "status_mechanism": "codex-notify",
        "credentials": { "supported": true, "imported": false }
    }]}))
    .expect("a kind from before the choices decodes");
    assert!(older.kinds[0].model.is_none() && older.kinds[0].effort.is_none());
}

#[test]
fn a_listed_package_carries_its_id_and_version_and_no_program() {
    let kind = AgentKind {
        id: "codex".to_owned(),
        name: "Codex".to_owned(),
        icon: "codex".to_owned(),
        owner: "ext:massorca.codex".to_owned(),
        installed: false,
        detected: false,
        program_path: None,
        package: KindPackage {
            state: PackageState::Available,
            id: Some("massorca.codex".to_owned()),
            version: Some("1.2.0".to_owned()),
        },
        resumable: true,
        status_mechanism: StatusMechanism::CodexNotify,
        credentials: KindCredentials {
            supported: true,
            imported: false,
        },
        problems: Vec::new(),
        permission_modes: Vec::new(),
        default_permission_mode: None,
        chat: None,
        model: None,
        fast: None,
        effort: None,
    };
    assert_wire(
        &AgentKinds { kinds: vec![kind] },
        &json!({ "kinds": [{
            "id": "codex",
            "name": "Codex",
            "icon": "codex",
            "owner": "ext:massorca.codex",
            "installed": false,
            "detected": false,
            "package": { "state": "available", "id": "massorca.codex", "version": "1.2.0" },
            "resumable": true,
            "status_mechanism": "codex-notify",
            "credentials": { "supported": true, "imported": false }
        }]}),
    );
}

#[test]
fn a_kind_from_before_detection_and_packages_still_decodes() {
    let before = json!({
        "id": "claude",
        "name": "Claude",
        "icon": "claude",
        "owner": "core",
        "installed": true,
        "program_path": "/usr/local/bin/claude",
        "resumable": true,
        "status_mechanism": "claude-settings",
        "credentials": { "supported": true, "imported": false }
    });
    let decoded: AgentKind =
        serde_json::from_value(before.clone()).expect("an agents/1 kind decodes");
    assert!(decoded.installed, "the old installed field still decodes");
    assert!(
        decoded.detected,
        "detected is absent, so it takes the value of installed"
    );
    assert_eq!(decoded.package, KindPackage::default());
    assert_eq!(decoded.package.state, PackageState::Unknown);

    let mut not_installed = before;
    not_installed["installed"] = json!(false);
    let decoded: AgentKind =
        serde_json::from_value(not_installed).expect("an agents/1 kind decodes");
    assert!(!decoded.installed);
    assert!(!decoded.detected, "installed false, so detected is false");
}

#[test]
fn an_unknown_package_state_decodes_as_unknown() {
    let decoded: KindPackage =
        serde_json::from_value(json!({ "state": "quarantined" })).expect("decodes");
    assert_eq!(decoded.state, PackageState::Unknown);
    assert_eq!(decoded.id, None);
}

#[test]
fn start_params_default_every_optional_field() {
    let decoded: AgentStartParams =
        serde_json::from_value(json!({ "workspace": "main", "kind": "fake" })).expect("decodes");
    assert_eq!(
        decoded,
        AgentStartParams {
            workspace: Some("main".to_owned()),
            kind: "fake".to_owned(),
            prompt: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: None,
            cols: None,
            rows: None,
            request_id: None,
            permission_mode: None,
            model: None,
            fast: None,
            effort: None,
        }
    );
}

/// A start with no workspace (decision 0094) sends no `workspace` key, so an older peer never sees a null it would
/// misread; a start that names one keeps the key.
#[test]
fn a_start_without_a_workspace_omits_the_key() {
    let no_project = AgentStartParams {
        workspace: None,
        kind: "fake".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: None,
        fast: None,
        effort: None,
    };
    assert_wire(&no_project, &json!({ "kind": "fake", "read_only": false }));
    let decoded: AgentStartParams =
        serde_json::from_value(json!({ "kind": "fake" })).expect("decodes");
    assert_eq!(decoded.workspace, None);
}

/// `model` and `effort` (decision 0094) are sent only when set, and decode from their keys.
#[test]
fn a_start_with_a_model_and_an_effort_carries_both_keys() {
    let chosen = AgentStartParams {
        workspace: Some("main".to_owned()),
        kind: "fake".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: Some("fast-model".to_owned()),
        fast: None,
        effort: Some("high".to_owned()),
    };
    assert_wire(
        &chosen,
        &json!({ "workspace": "main", "kind": "fake", "read_only": false, "model": "fast-model", "effort": "high" }),
    );
}

#[test]
fn bring_back_merges_and_prompts_unless_told_otherwise() {
    let decoded: AgentBringBackParams =
        serde_json::from_value(json!({ "child": "s-2" })).expect("decodes");
    assert!(decoded.merge && decoded.prompt);
    let declined: AgentBringBackParams =
        serde_json::from_value(json!({ "child": "s-2", "merge": false, "prompt": false }))
            .expect("decodes");
    assert!(!declined.merge && !declined.prompt);
}

#[test]
fn hook_payload_is_opaque_text() {
    assert_wire(
        &AgentHookParams {
            session: "s-1".to_owned(),
            token: "tok".to_owned(),
            event: "Stop".to_owned(),
            payload: Some("{\"session_id\":\"x\"}".to_owned()),
        },
        &json!({ "session": "s-1", "token": "tok", "event": "Stop", "payload": "{\"session_id\":\"x\"}" }),
    );
}

#[test]
fn watch_items_are_tagged_by_kind() {
    assert_wire(
        &AgentWatchEvent::Snapshot {
            sessions: Vec::new(),
        },
        &json!({ "kind": "snapshot", "sessions": [] }),
    );
    let unknown: AgentWatchEvent =
        serde_json::from_value(json!({ "kind": "reordered" })).expect("decodes");
    assert_eq!(unknown, AgentWatchEvent::Unknown);
}

#[test]
fn the_prompt_receipt_is_handed_to_terminal() {
    assert_wire(
        &PromptReceipt {
            receipt: ReceiptKind::HandedToTerminal,
            bytes: 12,
        },
        &json!({ "receipt": "handed_to_terminal", "bytes": 12 }),
    );
}

#[test]
fn values_from_a_newer_peer_decode_as_unknown() {
    let activity: Activity = serde_json::from_value(json!({
        "state": "thinking", "outcome": "partial", "source": "telepathy", "at_ms": 1
    }))
    .expect("decodes");
    assert_eq!(activity.state, ActivityState::Unknown);
    assert_eq!(activity.outcome, Some(Outcome::Unknown));
    assert_eq!(activity.source, ActivitySource::Unknown);
    let mechanism: StatusMechanism = serde_json::from_value(json!("acp-stream")).expect("decodes");
    assert_eq!(mechanism, StatusMechanism::Unknown);
}

#[test]
fn an_account_state_the_build_does_not_know_reads_as_unknown() {
    let known: AccountState = serde_json::from_value(json!("signed_in")).unwrap();
    assert_eq!(known, AccountState::SignedIn);
    let future: AccountState = serde_json::from_value(json!("refreshing")).unwrap();
    assert_eq!(future, AccountState::Unknown);
    assert_wire(&AccountState::NeedsSignIn, &json!("needs_sign_in"));
}

#[test]
fn an_account_summary_omits_an_absent_identity() {
    let summary = AccountSummary {
        id: "0195c0de-0000-7000-8000-000000000001".to_owned(),
        label: "work".to_owned(),
        state: AccountState::Migrated,
        identity: None,
        active: true,
        projects: vec!["p1".to_owned()],
        origin: AccountOrigin::Migrated,
    };
    assert_wire(
        &summary,
        &json!({
            "id": "0195c0de-0000-7000-8000-000000000001",
            "label": "work",
            "state": "migrated",
            "active": true,
            "projects": ["p1"],
            "origin": "migrated"
        }),
    );
}

#[test]
fn a_kind_lists_its_permission_modes_and_default_and_an_older_kind_has_none() {
    let mut kind: AgentKind = serde_json::from_value(json!({
        "id": "fake", "name": "Fake", "icon": "bot", "owner": "user", "installed": true,
        "resumable": false, "status_mechanism": "none",
        "credentials": { "supported": false, "imported": false }
    }))
    .expect("a kind from before permission modes decodes");
    assert!(kind.permission_modes.is_empty());
    assert_eq!(kind.default_permission_mode, None);
    assert!(
        serde_json::to_value(&kind)
            .unwrap()
            .get("permission_modes")
            .is_none(),
        "a kind with no modes omits them"
    );

    kind.permission_modes = vec![
        PermissionMode {
            id: "careful".to_owned(),
            label: "Careful".to_owned(),
            description: None,
            bypass: false,
            live_switch: false,
            may_fall_back: true,
            icon: None,
            tone: None,
            selectable: true,
        },
        PermissionMode {
            id: "wild".to_owned(),
            label: "Wild".to_owned(),
            description: Some("No prompts.".to_owned()),
            bypass: true,
            live_switch: true,
            may_fall_back: false,
            icon: None,
            tone: None,
            selectable: true,
        },
    ];
    kind.default_permission_mode = Some("careful".to_owned());
    let value = serde_json::to_value(&kind).unwrap();
    assert_eq!(
        value["permission_modes"],
        json!([
            { "id": "careful", "label": "Careful", "bypass": false, "live_switch": false, "may_fall_back": true },
            { "id": "wild", "label": "Wild", "description": "No prompts.", "bypass": true, "live_switch": true, "may_fall_back": false }
        ])
    );
    assert_eq!(value["default_permission_mode"], "careful");
    let decoded: AgentKind = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, kind);
    let bare: PermissionMode = serde_json::from_value(json!({ "id": "x", "label": "X" })).unwrap();
    assert!(
        !bare.bypass && !bare.live_switch && !bare.may_fall_back,
        "absent flags read as false"
    );
}

#[test]
fn a_session_carries_its_permission_mode_and_an_older_one_decodes_without() {
    let mut session = live_session();
    let before = serde_json::to_value(&session).unwrap();
    assert!(before.get("permission_mode").is_none());
    assert!(before.get("mode_confirmed").is_none(), "false is omitted");
    let decoded: AgentSession = serde_json::from_value(before).unwrap();
    assert_eq!(decoded.permission_mode, None);
    assert!(!decoded.mode_confirmed);

    session.permission_mode = Some("careful".to_owned());
    session.mode_confirmed = true;
    let value = serde_json::to_value(&session).unwrap();
    assert_eq!(value["permission_mode"], "careful");
    assert_eq!(value["mode_confirmed"], true);
    assert_eq!(
        serde_json::from_value::<AgentSession>(value).unwrap(),
        session
    );
}

#[test]
fn start_and_delegate_take_an_optional_permission_mode() {
    let start: AgentStartParams = serde_json::from_value(
        json!({ "workspace": "main", "kind": "fake", "permission_mode": "wild" }),
    )
    .unwrap();
    assert_eq!(start.permission_mode.as_deref(), Some("wild"));
    let old: AgentStartParams =
        serde_json::from_value(json!({ "workspace": "main", "kind": "fake" })).unwrap();
    assert_eq!(old.permission_mode, None);
    assert!(
        serde_json::to_value(&old)
            .unwrap()
            .get("permission_mode")
            .is_none()
    );

    let delegate: AgentDelegateParams = serde_json::from_value(
        json!({ "parent": "p", "prompt": "go", "permission_mode": "careful" }),
    )
    .unwrap();
    assert_eq!(delegate.permission_mode.as_deref(), Some("careful"));
    let old: AgentDelegateParams =
        serde_json::from_value(json!({ "parent": "p", "prompt": "go" })).unwrap();
    assert_eq!(old.permission_mode, None);
}

#[test]
fn set_mode_says_when_the_mode_applies_and_resume_takes_an_optional_mode() {
    let params: AgentSetModeParams =
        serde_json::from_value(json!({ "session": "s-1", "permission_mode": "wild" })).unwrap();
    assert_eq!(params.permission_mode, "wild");
    let mut session = live_session();
    session.permission_mode = Some("wild".to_owned());
    let set = AgentModeSet {
        applied: ModeApplied::Live,
        session: session.clone(),
    };
    let value = serde_json::to_value(&set).unwrap();
    assert_eq!(value["applied"], "live");
    assert_eq!(value["session"]["permission_mode"], "wild");
    assert_eq!(serde_json::from_value::<AgentModeSet>(value).unwrap(), set);
    assert_eq!(
        serde_json::to_value(ModeApplied::NextResume).unwrap(),
        "next_resume"
    );
    assert_eq!(
        serde_json::from_value::<ModeApplied>(json!("on_reboot")).unwrap(),
        ModeApplied::Unknown
    );

    let old: AgentResumeParams = serde_json::from_value(json!({ "session": "s-1" })).unwrap();
    assert_eq!(
        old.permission_mode, None,
        "a resume from before modes decodes"
    );
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        json!({ "session": "s-1" })
    );
}

#[test]
fn a_session_says_where_its_mode_came_from_and_what_waits_for_the_resume() {
    let mut session = live_session();
    session.mode_source = Some(ModeSource::VendorDefault);
    let value = serde_json::to_value(&session).unwrap();
    assert_eq!(value["mode_source"], "vendor_default");
    assert!(value.get("permission_mode").is_none(), "no mode is claimed");
    session.permission_mode = Some("careful".to_owned());
    session.mode_source = Some(ModeSource::Chosen);
    session.pending_permission_mode = Some("wild".to_owned());
    let value = serde_json::to_value(&session).unwrap();
    assert_eq!(value["mode_source"], "chosen");
    assert_eq!(value["pending_permission_mode"], "wild");
    assert_eq!(
        serde_json::from_value::<AgentSession>(value).unwrap(),
        session
    );
    assert_eq!(
        serde_json::from_value::<ModeSource>(json!("saved_preference")).unwrap(),
        ModeSource::Unknown
    );
    let old = serde_json::to_value(live_session()).unwrap();
    let decoded: AgentSession = serde_json::from_value(old).unwrap();
    assert_eq!(
        (decoded.mode_source, decoded.pending_permission_mode),
        (None, None)
    );
}

#[test]
fn an_activity_carries_when_it_was_seen_and_an_older_one_decodes_without() {
    let older = json!({ "state": "waiting", "source": "hook", "at_ms": 5 });
    let decoded: Activity =
        serde_json::from_value(older.clone()).expect("decodes without the field");
    assert_eq!(decoded.seen_at_ms, None, "absent means not seen");
    assert_wire(&decoded, &older);

    let seen = Activity {
        seen_at_ms: Some(9),
        ..decoded
    };
    assert_wire(
        &seen,
        &json!({ "state": "waiting", "source": "hook", "at_ms": 5, "seen_at_ms": 9 }),
    );
}

fn queued(state: agents::QueueState, reason: Option<&str>) -> agents::QueuedPrompt {
    agents::QueuedPrompt {
        id: "q-1".to_owned(),
        session: "s-1".to_owned(),
        position: 1,
        text: "next".to_owned(),
        state,
        reason: reason.map(str::to_owned),
        created_at_ms: 1,
        updated_at_ms: 2,
    }
}

#[test]
fn a_queued_prompt_says_its_state_and_reason_and_an_absent_reason_is_omitted() {
    assert_wire(
        &queued(agents::QueueState::Waiting, Some(agents::QUEUE_REASON_BUSY)),
        &json!({
            "id": "q-1", "session": "s-1", "position": 1, "text": "next",
            "state": "waiting", "reason": "busy", "created_at_ms": 1, "updated_at_ms": 2
        }),
    );
    assert_wire(
        &queued(agents::QueueState::Sending, None),
        &json!({
            "id": "q-1", "session": "s-1", "position": 1, "text": "next",
            "state": "sending", "created_at_ms": 1, "updated_at_ms": 2
        }),
    );
    let failed: agents::QueuedPrompt = serde_json::from_value(json!({
        "id": "q-1", "session": "s-1", "position": 1, "text": "next",
        "state": "failed", "reason": "the run has exited", "created_at_ms": 1, "updated_at_ms": 2
    }))
    .unwrap();
    assert_eq!(failed.state, agents::QueueState::Failed);
    assert_eq!(failed.reason.as_deref(), Some("the run has exited"));
    let newer: agents::QueuedPrompt = serde_json::from_value(json!({
        "id": "q-1", "session": "s-1", "position": 1, "text": "next",
        "state": "parked", "created_at_ms": 1, "updated_at_ms": 2
    }))
    .unwrap();
    assert_eq!(newer.state, agents::QueueState::Unknown);
}

#[test]
fn queue_params_and_the_list_have_their_wire_shapes() {
    assert_wire(
        &AgentQueueParams {
            session: "s-1".to_owned(),
            text: "later".to_owned(),
            request_id: None,
        },
        &json!({ "session": "s-1", "text": "later" }),
    );
    assert_wire(
        &AgentQueueParams {
            session: "s-1".to_owned(),
            text: "later".to_owned(),
            request_id: Some("req-1".to_owned()),
        },
        &json!({ "session": "s-1", "text": "later", "request_id": "req-1" }),
    );
    assert_wire(
        &AgentQueueEditParams {
            id: "q-1".to_owned(),
            text: "edited".to_owned(),
        },
        &json!({ "id": "q-1", "text": "edited" }),
    );
    assert_wire(
        &AgentQueueItemRef {
            id: "q-1".to_owned(),
        },
        &json!({ "id": "q-1" }),
    );
    assert_wire(
        &AgentQueueList {
            session: "s-1".to_owned(),
            items: vec![queued(agents::QueueState::Failed, Some("gone"))],
        },
        &json!({
            "session": "s-1",
            "items": [{
                "id": "q-1", "session": "s-1", "position": 1, "text": "next",
                "state": "failed", "reason": "gone", "created_at_ms": 1, "updated_at_ms": 2
            }]
        }),
    );
    assert_eq!(agents::MAX_QUEUED_PROMPTS, 100);
    assert_eq!(agents::QUEUE, "agent.queue");
    assert_eq!(agents::QUEUE_LIST, "agent.queue.list");
    assert_eq!(agents::QUEUE_EDIT, "agent.queue.edit");
    assert_eq!(agents::QUEUE_REMOVE, "agent.queue.remove");
    assert_eq!(agents::QUEUE_SEND_NOW, "agent.queue.send_now");
    assert_eq!(agents::QUEUE_RESUME, "agent.queue.resume");
}

#[test]
fn a_queued_prompt_does_not_print_its_text_in_debug() {
    let item = queued(agents::QueueState::Waiting, Some(agents::QUEUE_REASON_BUSY));
    let shown = format!("{item:?}");
    assert!(!shown.contains("next"), "the text is not printed: {shown}");
    assert!(
        shown.contains("q-1") && shown.contains("Waiting"),
        "{shown}"
    );
    assert_eq!(agents::MAX_DELIVERED_REQUEST_IDS, 100);
}

#[test]
fn mark_seen_takes_an_optional_activity_time_and_an_older_caller_sends_only_the_session() {
    let older: agents::AgentMarkSeenParams =
        serde_json::from_value(json!({ "session": "s-1" })).expect("decodes without at_ms");
    assert_eq!(older.at_ms, None);
    assert_wire(&older, &json!({ "session": "s-1" }));
    assert_wire(
        &agents::AgentMarkSeenParams {
            session: "s-1".to_owned(),
            at_ms: Some(7),
        },
        &json!({ "session": "s-1", "at_ms": 7 }),
    );
}

#[test]
fn release_and_retain_flags_are_absent_until_set_and_an_archive_reads_back() {
    let wire = serde_json::to_value(live_session()).unwrap();
    assert!(
        wire.get("retained").is_none() && wire.get("released").is_none(),
        "{wire}"
    );

    let archived = AgentArchive {
        session: "s1".to_owned(),
        released: true,
        text: Some("tail".to_owned()),
        truncated: true,
        withheld: false,
    };
    assert_wire(
        &archived,
        &json!({ "session": "s1", "released": true, "text": "tail", "truncated": true }),
    );
    let withheld = AgentArchive {
        session: "s1".to_owned(),
        released: true,
        text: None,
        truncated: false,
        withheld: true,
    };
    assert_wire(
        &withheld,
        &json!({ "session": "s1", "released": true, "truncated": false, "withheld": true }),
    );
}

#[test]
fn import_candidates_and_an_import_without_a_folder_omit_absent_fields() {
    use agents::{AccountsImportParams, CandidateFile, ImportCandidate, ImportCandidates};

    assert_wire(
        &AccountsImportParams {
            kind: "claude".to_owned(),
            from: None,
            label: None,
        },
        &json!({ "kind": "claude" }),
    );
    assert_wire(&agents::ImportCandidatesParams::default(), &json!({}));
    let candidates = ImportCandidates {
        candidates: vec![ImportCandidate {
            kind: "claude".to_owned(),
            ready: true,
            folder: Some("<home>/.claude".to_owned()),
            files: vec![
                CandidateFile {
                    name: ".credentials.json".to_owned(),
                    required: true,
                    found: true,
                    path: Some("<home>/.claude/.credentials.json".to_owned()),
                    env: Some("CLAUDE_CONFIG_DIR".to_owned()),
                    reason: None,
                },
                CandidateFile {
                    name: ".claude.json".to_owned(),
                    required: false,
                    found: false,
                    path: None,
                    env: None,
                    reason: Some("a link".to_owned()),
                },
            ],
        }],
    };
    assert_wire(
        &candidates,
        &json!({ "candidates": [{
            "kind": "claude",
            "ready": true,
            "folder": "<home>/.claude",
            "files": [
                {
                    "name": ".credentials.json", "required": true, "found": true,
                    "path": "<home>/.claude/.credentials.json", "env": "CLAUDE_CONFIG_DIR"
                },
                { "name": ".claude.json", "required": false, "found": false, "reason": "a link" }
            ]
        }] }),
    );
}

#[test]
fn an_import_that_updated_an_account_names_it_and_a_new_one_omits_the_field() {
    let updated: agents::AccountsImported = serde_json::from_value(json!({
        "result": "imported",
        "updated": "0195c0de-0000-7000-8000-000000000002"
    }))
    .expect("an updated import decodes");
    assert_eq!(
        updated.updated.as_deref(),
        Some("0195c0de-0000-7000-8000-000000000002")
    );
    let older: agents::AccountsImported =
        serde_json::from_value(json!({ "result": "imported" })).expect("an older result decodes");
    assert_eq!(older.updated, None);
    assert_eq!(
        serde_json::to_value(&older).expect("encodes"),
        json!({ "result": "imported" }),
        "a new account's result has no `updated`"
    );
}

#[test]
fn a_session_says_which_children_it_could_delegate_and_an_older_one_says_nothing() {
    use massorca_protocol::methods::agents::{DelegationOptions, WritingChild};

    let mut session = live_session();
    session.delegation = Some(Box::new(DelegationOptions {
        writing_child: WritingChild::Unavailable,
        read_only_child: false,
        reason: Some("at the depth limit".to_owned()),
    }));
    session.notes = vec!["works in this folder".to_owned()];
    let value = serde_json::to_value(&session).expect("serializes");
    assert_eq!(
        value["delegation"],
        json!({
            "writing_child": "unavailable",
            "read_only_child": false,
            "reason": "at the depth limit"
        })
    );
    assert_eq!(value["notes"], json!(["works in this folder"]));
    let decoded: AgentSession = serde_json::from_value(value).expect("deserializes");
    assert_eq!(decoded, session);

    for (child, wire) in [
        (WritingChild::OwnWorktree, "own_worktree"),
        (WritingChild::SharedFolder, "shared_folder"),
    ] {
        assert_eq!(serde_json::to_value(child).expect("serializes"), wire);
    }
    let newer: WritingChild = serde_json::from_value(json!("elsewhere")).expect("decodes");
    assert_eq!(newer, WritingChild::Unknown);

    // A record from an older peer has neither field.
    let mut older = serde_json::to_value(live_session()).expect("serializes");
    assert!(older.get("delegation").is_none());
    older.as_object_mut().expect("an object").remove("notes");
    let decoded: AgentSession = serde_json::from_value(older).expect("an older record decodes");
    assert_eq!(decoded.delegation, None);
    assert!(decoded.notes.is_empty());
}

#[test]
fn a_models_listing_names_each_models_source_and_omits_an_absent_time_and_error() {
    let declared = AgentModel {
        id: "sonnet".to_owned(),
        label: "Sonnet".to_owned(),
        default: true,
        efforts: Some(vec!["low".to_owned(), "high".to_owned()]),
        source: ModelSource::Declared,
    };
    let discovered = AgentModel {
        id: "opus[1m]".to_owned(),
        label: "Opus (1M context)".to_owned(),
        default: false,
        efforts: None,
        source: ModelSource::Discovered,
    };
    assert_wire(
        &AgentModels {
            models: vec![declared, discovered],
            fetched_at_ms: Some(1_700_000_000_000),
            error: None,
        },
        &json!({
            "models": [
                {
                    "id": "sonnet",
                    "label": "Sonnet",
                    "default": true,
                    "efforts": ["low", "high"],
                    "source": "declared"
                },
                { "id": "opus[1m]", "label": "Opus (1M context)", "source": "discovered" }
            ],
            "fetched_at_ms": 1_700_000_000_000u64
        }),
    );
    assert_wire(
        &AgentModels {
            models: Vec::new(),
            fetched_at_ms: None,
            error: Some(ModelListingError {
                code: ModelListingFailure::CliMissing,
                message: "the program was not found".to_owned(),
            }),
        },
        &json!({
            "models": [],
            "error": { "code": "cli_missing", "message": "the program was not found" }
        }),
    );
}

#[test]
fn the_models_params_name_a_kind_and_omit_an_absent_account_and_an_unset_refresh() {
    assert_wire(
        &AgentModelsParams {
            kind: "claude".to_owned(),
            account: None,
            refresh: false,
        },
        &json!({ "kind": "claude" }),
    );
    assert_wire(
        &AgentModelsParams {
            kind: "claude".to_owned(),
            account: Some("acc-1".to_owned()),
            refresh: true,
        },
        &json!({ "kind": "claude", "account": "acc-1", "refresh": true }),
    );
}

#[test]
fn every_listing_failure_has_its_snake_case_code() {
    let codes: Vec<Value> = [
        ModelListingFailure::CliMissing,
        ModelListingFailure::NotSignedIn,
        ModelListingFailure::TimedOut,
        ModelListingFailure::Failed,
        ModelListingFailure::NoModels,
    ]
    .into_iter()
    .map(|code| serde_json::to_value(code).expect("a failure encodes"))
    .collect();
    assert_eq!(
        codes,
        [
            json!("cli_missing"),
            json!("not_signed_in"),
            json!("timed_out"),
            json!("failed"),
            json!("no_models"),
        ]
    );
}

#[test]
fn a_mode_carries_its_icon_tone_and_whether_it_is_offered_and_an_older_one_reads_as_offered() {
    let mode = PermissionMode {
        id: "hush".to_owned(),
        label: "Hush".to_owned(),
        description: None,
        bypass: false,
        live_switch: false,
        may_fall_back: false,
        icon: Some("ban".to_owned()),
        tone: Some(ModeTone::Normal),
        selectable: false,
    };
    assert_wire(
        &mode,
        &json!({
            "id": "hush", "label": "Hush", "bypass": false, "live_switch": false, "may_fall_back": false,
            "icon": "ban", "tone": "normal", "selectable": false
        }),
    );
    let older: PermissionMode = serde_json::from_value(json!({ "id": "x", "label": "X" })).unwrap();
    assert!(older.selectable, "an older peer's mode is offered");
    assert_eq!((older.icon, older.tone), (None, None));
    let offered = serde_json::to_value(PermissionMode {
        selectable: true,
        tone: Some(ModeTone::Warning),
        ..mode
    })
    .unwrap();
    assert!(
        offered.get("selectable").is_none(),
        "an offered mode omits it"
    );
    assert_eq!(offered["tone"], "warning");
    let newer: PermissionMode =
        serde_json::from_value(json!({ "id": "x", "label": "X", "tone": "loud" })).unwrap();
    assert_eq!(newer.tone, Some(ModeTone::Unknown));
}

#[test]
fn archive_and_list_params_and_the_archived_flag() {
    assert_wire(
        &AgentArchiveSessionParams {
            session: "s1".to_owned(),
            archived: true,
        },
        &json!({ "session": "s1", "archived": true }),
    );
    assert_wire(
        &AgentListParams {
            workspace: None,
            include_archived: true,
        },
        &json!({ "include_archived": true }),
    );
    let older: AgentListParams = serde_json::from_value(json!({})).unwrap();
    assert!(!older.include_archived);
    let mut session = live_session();
    assert!(
        serde_json::to_value(&session)
            .unwrap()
            .get("archived")
            .is_none()
    );
    session.archived = true;
    let value = serde_json::to_value(&session).unwrap();
    assert_eq!(value["archived"], true);
    assert_eq!(
        serde_json::from_value::<AgentSession>(value).unwrap(),
        session
    );
}

#[test]
fn forget_takes_optional_workspace_removal_and_answers_with_the_session_flattened() {
    let older: AgentForgetParams = serde_json::from_value(json!({ "session": "s1" })).unwrap();
    assert!(!older.remove_workspace && !older.force && !older.discard_ignored);
    assert_eq!(older.expect_dirty, None);
    assert_wire(
        &AgentForgetParams {
            session: "s1".to_owned(),
            remove_workspace: true,
            force: true,
            discard_ignored: true,
            expect_dirty: Some(false),
            allow_failed_archive: true,
        },
        &json!({
            "session": "s1", "remove_workspace": true, "force": true, "discard_ignored": true,
            "expect_dirty": false, "allow_failed_archive": true
        }),
    );
    let forgotten = AgentForgotten {
        session: live_session(),
        workspace_removed: None,
        workspace_error: Some(WorkspaceRemovalError {
            code: "workspace.locked".to_owned(),
            message: "locked".to_owned(),
        }),
    };
    let value = serde_json::to_value(&forgotten).unwrap();
    assert_eq!(
        value["id"],
        forgotten.session.id.as_str(),
        "the session is flattened"
    );
    assert_eq!(value["workspace_error"]["code"], "workspace.locked");
    assert!(value.get("workspace_removed").is_none());
    assert_eq!(
        serde_json::from_value::<AgentForgotten>(value.clone()).unwrap(),
        forgotten
    );
    let as_session: AgentSession = serde_json::from_value(value).unwrap();
    assert_eq!(
        as_session, forgotten.session,
        "an older caller reads a session"
    );
}

#[test]
fn a_forget_preview_names_its_workspace_and_omits_what_is_absent() {
    let preview = AgentForgetPreview {
        session: "s1".to_owned(),
        can_forget: true,
        forget_blocker: None,
        workspace: ForgetPreviewWorkspace {
            id: "w1".to_owned(),
            kind: WorkspaceKind::Worktree,
            path: None,
            branch: Some("feature".to_owned()),
            removable: false,
            blocker: Some("used by s2".to_owned()),
            blocker_code: Some("agent.busy".to_owned()),
            dirty: true,
            untracked: false,
            ignored_files: false,
            other_sessions: vec!["s2".to_owned()],
            other_teams: Vec::new(),
            other_tasks: vec!["t1".to_owned()],
            running_terminals: Vec::new(),
        },
    };
    assert_wire(
        &preview,
        &json!({
            "session": "s1",
            "can_forget": true,
            "workspace": {
                "id": "w1", "kind": "worktree", "branch": "feature", "removable": false,
                "blocker": "used by s2", "blocker_code": "agent.busy", "dirty": true, "untracked": false,
                "other_sessions": ["s2"], "other_tasks": ["t1"]
            }
        }),
    );
}

#[test]
fn rename_takes_a_session_and_a_title() {
    use massorca_protocol::methods::agents::{AgentRenameParams, MAX_TITLE_CHARS, RENAME};
    assert_eq!(RENAME, "agent.rename");
    assert_eq!(MAX_TITLE_CHARS, 120);
    assert_wire(
        &AgentRenameParams {
            session: "s1".to_owned(),
            title: "New name".to_owned(),
        },
        &json!({ "session": "s1", "title": "New name" }),
    );
}

/// Fast mode (decision 0165): a start asks for it, a kind and a model say whether they offer it, and a session says
/// whether it was asked for. Each is absent when off, and an older peer decodes without them.
#[test]
fn fast_mode_is_asked_on_a_start_offered_by_kinds_and_models_and_shown_on_a_session() {
    let asked = AgentStartParams {
        workspace: Some("main".to_owned()),
        kind: "claude".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: Some("opus".to_owned()),
        effort: None,
        fast: Some(true),
    };
    assert_wire(
        &asked,
        &json!({ "workspace": "main", "kind": "claude", "read_only": false, "model": "opus", "fast": true }),
    );
    let plain = AgentStartParams {
        fast: None,
        model: None,
        ..asked
    };
    assert_wire(
        &plain,
        &json!({ "workspace": "main", "kind": "claude", "read_only": false }),
    );

    let choices = KindModel {
        choices: vec![
            KindModelChoice {
                id: "opus".to_owned(),
                label: "Opus".to_owned(),
                default: false,
                efforts: None,
                fast: true,
            },
            KindModelChoice {
                id: "haiku".to_owned(),
                label: "Haiku".to_owned(),
                default: false,
                efforts: None,
                fast: false,
            },
        ],
    };
    assert_wire(
        &choices,
        &json!({ "choices": [
            { "id": "opus", "label": "Opus", "fast": true },
            { "id": "haiku", "label": "Haiku" }
        ]}),
    );

    let kinds: AgentKinds = serde_json::from_value(json!({ "kinds": [
        {
            "id": "claude", "name": "Claude", "icon": "claude", "owner": "core",
            "installed": true, "resumable": true, "status_mechanism": "claude-settings",
            "credentials": { "supported": true, "imported": false },
            "fast": { "supported": true }
        },
        {
            "id": "codex", "name": "Codex", "icon": "codex", "owner": "core",
            "installed": false, "resumable": true, "status_mechanism": "codex-notify",
            "credentials": { "supported": true, "imported": false },
            "fast": { "supported": false, "reason": "the kind declares no fast mode" }
        },
        {
            "id": "old", "name": "Old", "icon": "bot", "owner": "core",
            "installed": false, "resumable": false, "status_mechanism": "none",
            "credentials": { "supported": false, "imported": false }
        }
    ]}))
    .expect("kinds decode with and without fast mode");
    assert_eq!(
        kinds.kinds[0].fast,
        Some(massorca_protocol::methods::agents::KindFast {
            supported: true,
            reason: None
        })
    );
    assert_eq!(
        kinds.kinds[1]
            .fast
            .as_ref()
            .and_then(|fast| fast.reason.as_deref()),
        Some("the kind declares no fast mode")
    );
    assert!(kinds.kinds[2].fast.is_none());

    let session: AgentSession = serde_json::from_value(json!({
        "id": "s-1", "kind": "claude", "workspace_id": "w-1", "read_only": false,
        "created_at_ms": 1, "execution": { "state": "live" },
        "fast": { "on": true, "confirmed": false }
    }))
    .expect("a session with fast mode decodes");
    assert_eq!(
        session.fast,
        Some(massorca_protocol::methods::agents::SessionFast {
            on: true,
            confirmed: false
        })
    );
}

#[test]
fn shared_folder_with_lists_the_other_writers_and_is_absent_when_empty() {
    let shared = AgentSession {
        shared_folder_with: vec!["s-2".to_owned()],
        ..live_session()
    };
    let value = serde_json::to_value(shared.clone()).expect("serializes");
    assert_eq!(value["shared_folder_with"], json!(["s-2"]));
    let decoded: AgentSession = serde_json::from_value(value).expect("deserializes");
    assert_eq!(decoded, shared);

    let alone = serde_json::to_value(live_session()).expect("serializes");
    assert!(alone.get("shared_folder_with").is_none(), "{alone}");
    let older: AgentSession = serde_json::from_value(alone).expect("an older record decodes");
    assert!(older.shared_folder_with.is_empty());
}
