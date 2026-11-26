//! The agent tools a skill can be placed for, and the folder each reads at global scope (pv96's
//! `skill-install-providers.ts`). An agent that reads the canonical `.agents/skills` root needs no folder of its own.

use super::discover::SkillSource;

/// An agent tool a skill can be placed for. The ids match pv96's, so one id names one tool in both apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacementAgent {
    Codex,
    Claude,
    Cursor,
    Gemini,
    Droid,
    Continue,
    Trae,
    Grok,
    Aug,
    Muse,
    Zcode,
}

impl PlacementAgent {
    /// Every agent, in pv96's registry order.
    pub const ALL: [PlacementAgent; 11] = [
        PlacementAgent::Codex,
        PlacementAgent::Claude,
        PlacementAgent::Cursor,
        PlacementAgent::Gemini,
        PlacementAgent::Droid,
        PlacementAgent::Continue,
        PlacementAgent::Trae,
        PlacementAgent::Grok,
        PlacementAgent::Aug,
        PlacementAgent::Muse,
        PlacementAgent::Zcode,
    ];

    /// The id the CLI, the protocol and the record use.
    pub fn id(self) -> &'static str {
        match self {
            PlacementAgent::Codex => "codex",
            PlacementAgent::Claude => "claude",
            PlacementAgent::Cursor => "cursor",
            PlacementAgent::Gemini => "gemini",
            PlacementAgent::Droid => "droid",
            PlacementAgent::Continue => "continue",
            PlacementAgent::Trae => "trae",
            PlacementAgent::Grok => "grok",
            PlacementAgent::Aug => "aug",
            PlacementAgent::Muse => "muse",
            PlacementAgent::Zcode => "zcode",
        }
    }

    /// The agent with this id, or `None` for an id this build does not know.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.id() == id)
    }

    /// The folder the agent reads below the home folder, or `None` when it reads the canonical root instead.
    pub fn home_folder(self) -> Option<&'static [&'static str]> {
        match self {
            PlacementAgent::Codex | PlacementAgent::Muse => None,
            PlacementAgent::Claude => Some(&[".claude", "skills"]),
            PlacementAgent::Cursor => Some(&[".cursor", "skills"]),
            PlacementAgent::Gemini => Some(&[".gemini", "skills"]),
            PlacementAgent::Droid => Some(&[".factory", "skills"]),
            PlacementAgent::Continue => Some(&[".continue", "skills"]),
            PlacementAgent::Trae => Some(&[".trae-cn", "skills"]),
            PlacementAgent::Grok => Some(&[".grok", "skills"]),
            PlacementAgent::Aug => Some(&[".augment", "skills"]),
            PlacementAgent::Zcode => Some(&[".zcode", "skills"]),
        }
    }
}

/// The agents whose skill root exists on this host, in the order discovery lists them. An agent whose folder is missing is
/// not installed here, so nothing is created for it.
pub fn detected_agents(sources: &[SkillSource]) -> Vec<PlacementAgent> {
    let mut agents = Vec::new();
    for source in sources.iter().filter(|source| source.exists) {
        if let Some(agent) = source.owner.as_deref().and_then(PlacementAgent::from_id)
            && !agents.contains(&agent)
        {
            agents.push(agent);
        }
    }
    agents
}
