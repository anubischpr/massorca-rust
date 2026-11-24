//! Skill discovery (SKL-01): the skill files on this host and what each one says about itself. It only reads; nothing
//! is persisted.

mod frontmatter;
mod walk;

pub use frontmatter::{SkillSummary, read_skill_summary};
pub use walk::find_skill_files;
