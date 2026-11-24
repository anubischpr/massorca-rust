//! The `skills.discover` wire shapes: withheld paths are left out, and a value a newer peer adds decodes as `Unknown`.

use massorca_protocol::methods::skills::{
    DISCOVER, DiscoveredSkill, SkillDiscovery, SkillProvider, SkillSource, SkillSourceKind,
    SkillsDiscoverParams, SkipReason,
};
use serde_json::json;

#[test]
fn the_method_is_named_and_empty_params_mean_every_kind_and_a_cached_scan() {
    assert_eq!(DISCOVER, "skills.discover");

    let params: SkillsDiscoverParams = serde_json::from_value(json!({})).expect("empty params");

    assert!(!params.refresh);
    assert!(params.source_kinds.is_empty());
}

#[test]
fn a_kind_or_provider_or_reason_from_a_newer_peer_decodes_as_unknown() {
    let kind: SkillSourceKind = serde_json::from_value(json!("future")).expect("decodes");
    let reason: SkipReason = serde_json::from_value(json!("elsewhere")).expect("decodes");

    assert_eq!(kind, SkillSourceKind::Unknown);
    assert_eq!(reason, SkipReason::Unknown);
    assert_eq!(
        serde_json::to_value(SkillProvider::AgentSkills).expect("encodes"),
        json!("agent-skills")
    );
}

#[test]
fn withheld_paths_are_left_out_and_the_discovery_reads_back() {
    let discovery = SkillDiscovery {
        skills: vec![DiscoveredSkill {
            id: "0123456789abcdef".to_owned(),
            name: "alpha".to_owned(),
            description: None,
            providers: vec![SkillProvider::Codex],
            source_kind: SkillSourceKind::Home,
            source_label: "Codex home".to_owned(),
            root_path: None,
            root_paths: Vec::new(),
            directory_path: None,
            skill_file_path: None,
            installed: true,
            updated_at: None,
            disable_slash_command: false,
        }],
        sources: vec![SkillSource {
            id: "home-codex".to_owned(),
            label: "Codex home".to_owned(),
            path: None,
            source_kind: SkillSourceKind::Home,
            providers: vec![SkillProvider::Codex],
            owner: Some("codex".to_owned()),
            exists: true,
            skipped_reason: None,
        }],
        scanned_at: 1,
    };

    let value = serde_json::to_value(&discovery).expect("encodes");

    assert!(value["skills"][0].get("root_path").is_none());
    assert!(value["sources"][0].get("path").is_none());
    let back: SkillDiscovery = serde_json::from_value(value).expect("decodes");
    assert_eq!(back, discovery);
}

#[test]
fn a_root_cut_off_by_the_scan_budget_is_truncated_on_the_wire() {
    let reason: SkipReason = serde_json::from_value(json!("truncated")).expect("decodes");

    assert_eq!(reason, SkipReason::Truncated);
    assert_eq!(
        serde_json::to_value(SkipReason::Truncated).expect("encodes"),
        json!("truncated")
    );
}
