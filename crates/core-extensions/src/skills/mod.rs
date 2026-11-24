//! Skill discovery (SKL-01): the skill files on this host and what each one says about itself. It only reads; nothing
//! is persisted.

mod claude_plugins;
mod discover;
mod frontmatter;
mod roots;
mod walk;

pub use discover::{
    DiscoveredSkill, DiscoveryRequest, MAX_SCAN_ENTRIES, SkillDiscovery, SkillSource, SkipReason,
    discover,
};
pub use frontmatter::{SkillSummary, read_skill_summary};
pub use roots::{SkillProvider, SkillRootEnv, SkillSourceKind};
pub use walk::{Walk, find_skill_files};
