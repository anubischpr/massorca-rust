//! Skill discovery (SKL-01): the skill files on this host and what each one says about itself. It only reads; nothing
//! is persisted. Placement (SKL-02) and the skills MassOrca ships (SKL-04) write into agent folders, through the record.

mod bundled;
mod claude_plugins;
mod discover;
mod frontmatter;
mod placement;
mod placement_agents;
mod placement_error;
mod placement_link;
mod placement_model;
mod placement_record;
mod placement_tree;
mod roots;
mod walk;

pub use bundled::{
    BundledSkill, BundledState, BundledStatus, bundled_skills, bundled_status, bundled_statuses,
    find_bundled, install_bundled, refresh_bundled,
};
pub use placement::{
    managed_placements, place_skill, place_skill_in, place_skill_with, place_skill_with_in,
    preview_placement, preview_placement_in, switch_agents_in, unplace_skill, unplace_skill_in,
};
pub use placement_agents::{PlacementAgent, detected_agents};
pub use placement_error::PlacementError;
pub use placement_model::{
    AgentOutcome, LinkMode, ManagedAgent, ManagedSkill, Outcome, PlacementPreview, PlacementReport,
    PlacementScope, PlacementStore, Plan, PlannedAgent, SkipCause, Topology,
};

pub use discover::{
    DiscoveredSkill, DiscoveryRequest, MAX_SCAN_ENTRIES, SkillDiscovery, SkillSource, SkipReason,
    discover,
};
pub use frontmatter::{SkillSummary, read_skill_summary};
pub use roots::{SkillProvider, SkillRootEnv, SkillSourceKind};
pub use walk::{Walk, find_skill_files};
