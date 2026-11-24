//! Skill discovery (SKL-01): `skills.discover` lists the skills agent tools and registered folders keep on this host.
//! Read only; nothing is persisted. Every enum has an `Unknown` variant for values from a newer peer.

use serde::{Deserialize, Serialize};

/// Params: [`SkillsDiscoverParams`]. Result: [`SkillDiscovery`].
pub const DISCOVER: &str = "skills.discover";

/// Params of `skills.discover`. A scan is shared for ten seconds unless `refresh` asks for a fresh one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsDiscoverParams {
    #[serde(default)]
    pub refresh: bool,
    /// Only skills of these kinds; empty keeps every kind.
    #[serde(default)]
    pub source_kinds: Vec<SkillSourceKind>,
}

/// Result of `skills.discover`: each skill once, and each root with whether it was read and, if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDiscovery {
    pub skills: Vec<DiscoveredSkill>,
    pub sources: Vec<SkillSource>,
    /// Unix milliseconds of the scan.
    pub scanned_at: i64,
}

/// One skill on disk. The path fields are left out for a caller without host administration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub providers: Vec<SkillProvider>,
    pub source_kind: SkillSourceKind,
    pub source_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_file_path: Option<String>,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub disable_slash_command: bool,
}

/// One root a scan read or skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    pub id: String,
    pub label: String,
    /// The root's folder; left out for a caller without host administration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub source_kind: SkillSourceKind,
    pub providers: Vec<SkillProvider>,
    /// The agent that owns the root; absent for the shared `.agents` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<SkipReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    Home,
    Repo,
    Bundled,
    Plugin,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillProvider {
    Codex,
    Claude,
    AgentSkills,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Why a root was not read: it is missing, or it exists but cannot be listed as a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Missing,
    Unreadable,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}
