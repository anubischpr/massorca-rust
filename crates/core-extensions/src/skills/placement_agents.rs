//! The agent tools a skill can be placed for, and the folder each reads at global scope (pv96's
//! `skill-install-providers.ts`). An agent that reads the canonical `.agents/skills` root needs no folder of its own.

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
