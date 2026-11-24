//! Skill discovery (SKL-01): the `SKILL.md` folders agent tools keep in the user's home folder, found on this host.
//! It only reads; nothing is persisted.

mod discover;
mod frontmatter;
mod roots;
mod walk;

pub use discover::{
    DiscoveredSkill, DiscoveryRequest, SkillDiscovery, SkillSource, SkipReason, discover,
};
pub use frontmatter::{SkillSummary, read_skill_summary};
pub use roots::{SkillProvider, SkillRootEnv, SkillSourceKind};
pub use walk::find_skill_files;
