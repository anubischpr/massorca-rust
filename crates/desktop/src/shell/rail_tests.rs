//! The mark each desktop agent draws on the rail, the sidebar and the Marketplace cards: one mark per agent.

use massorca_ui::components::agent_icon::AgentKind;

use super::rail::agent_kind;
use crate::model::shell::Agent;

#[test]
fn each_desktop_agent_draws_its_own_mark() {
    assert_eq!(agent_kind(Agent::Claude), AgentKind::Claude);
    assert_eq!(agent_kind(Agent::Codex), AgentKind::Codex);
    assert_eq!(agent_kind(Agent::Antigravity), AgentKind::Antigravity);
    assert_eq!(agent_kind(Agent::GentleShell), AgentKind::GentleShell);
    assert_eq!(agent_kind(Agent::Gemini), AgentKind::Gemini);
    assert_eq!(agent_kind(Agent::OpenCode), AgentKind::OpenCode);
    assert_eq!(agent_kind(Agent::Cursor), AgentKind::Cursor);
    assert_eq!(agent_kind(Agent::Unknown), AgentKind::Unknown);
}
