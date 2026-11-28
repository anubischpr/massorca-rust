//! Bundled skills (SKL-04, capability `skills-bundled/1`): the skills MassOrca ships, with their state on this host. The list
//! only reads. `skills.bundled.install` and `skills.bundled.refresh` place the skill through the placement of SKL-02 and need
//! host administration; removing it is `skills.unplace`. A bundled skill is named, never given a path. Every enum has an
//! `Unknown` variant for values from a newer peer.

use serde::{Deserialize, Serialize};

use super::skill_placement::PlacementAgent;

/// Params: [`SkillsBundledListParams`]. Result: [`SkillsBundledListResult`]. Changes nothing.
pub const LIST: &str = "skills.bundled.list";
/// Params: [`SkillsBundledInstallParams`]. Result: the `skills.place` result, [`super::skill_placement::SkillPlacementResult`].
pub const INSTALL: &str = "skills.bundled.install";
/// Params: [`SkillsBundledNameParams`]. Result: the `skills.place` result, [`super::skill_placement::SkillPlacementResult`].
pub const REFRESH: &str = "skills.bundled.refresh";

/// Params of `skills.bundled.list`: none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsBundledListParams {}

/// Result of `skills.bundled.list`: every skill this build ships, in its order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsBundledListResult {
    pub skills: Vec<BundledSkill>,
}

/// One skill this build ships, with its state on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundledSkill {
    pub name: String,
    pub description: String,
    /// The MassOrca version this text was built into.
    pub version: String,
    pub state: BundledSkillState,
    /// The agents the skill is placed for; empty when it is not placed.
    #[serde(default)]
    pub agents: Vec<PlacementAgent>,
}

/// Where a bundled skill stands on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundledSkillState {
    /// MassOrca has not placed it on this host.
    NotPlaced,
    /// The placed copies hold this build's text.
    Current,
    /// The placed copies hold another build's text: `skills.bundled.refresh` updates them.
    Outdated,
    /// A value from a newer peer that this build does not know.
    #[serde(other)]
    Unknown,
}

/// Params of `skills.bundled.install`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsBundledInstallParams {
    /// The bundled skill's name, as `skills.bundled.list` shows it.
    pub name: String,
    /// The agents to place it for. Empty places it for every agent whose folder exists on this host.
    #[serde(default)]
    pub agents: Vec<PlacementAgent>,
}

/// Params of `skills.bundled.refresh`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsBundledNameParams {
    /// The bundled skill's name, as `skills.bundled.list` shows it.
    pub name: String,
}
