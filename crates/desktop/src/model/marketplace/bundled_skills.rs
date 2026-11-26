//! The MassOrca skills the Skills section lists above the discovered ones (SKL-04): each skill's state on This computer,
//! the one action its row offers, and the one sentence a review shows before an install or an update.

use massorca_protocol::methods::skill_bundled::{BundledSkill, BundledSkillState};
use massorca_protocol::methods::skill_placement::PlacementAgent;

use super::skill_placement::{agent_label, join_labels};

/// The one action a bundled skill's row offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundledAction {
    /// Places the skill for every agent folder on this computer.
    Install,
    /// Brings the placed copies to this build's text.
    Update,
    /// Removes the copies MassOrca placed; a copy the user changed stays.
    Remove,
}

/// One bundled skill as a row of the MassOrca skills group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledRow {
    pub name: String,
    pub description: String,
    /// "Not installed", "Installed for Claude Code and Cursor", or "Update available for Codex".
    pub state_line: String,
    /// The action the row offers; None when there is nothing to do with the skill here.
    pub action: Option<BundledAction>,
    /// The agents the skill is placed for, in the order the service lists them.
    pub agents: Vec<PlacementAgent>,
}

/// The rows of the bundled skills, in the order the service lists them.
pub fn bundled_rows(skills: &[BundledSkill]) -> Vec<BundledRow> {
    skills.iter().map(bundled_row).collect()
}

/// The one sentence a review shows before an install or an update, naming the agent tools it writes to.
pub fn bundled_review_sentence(name: &str, update: bool, agents: &[PlacementAgent]) -> String {
    let labels: Vec<&str> = agents.iter().map(|agent| agent_label(*agent)).collect();
    match (update, labels.is_empty()) {
        (false, true) => format!("There is no agent folder on this computer to install {name} in."),
        (false, false) => format!("Install {name} for {}?", join_labels(&labels)),
        (true, true) => {
            format!("{name} is not placed for any agent tool, so there is nothing to update.")
        }
        (true, false) => format!(
            "Update {name} for {} to this MassOrca version?",
            join_labels(&labels)
        ),
    }
}

fn bundled_row(skill: &BundledSkill) -> BundledRow {
    BundledRow {
        name: skill.name.clone(),
        description: skill.description.clone(),
        state_line: state_line(skill.state, &skill.agents),
        action: action(skill.state, &skill.agents),
        agents: skill.agents.clone(),
    }
}

/// The state a row shows, with the agent tools the skill is placed for.
fn state_line(state: BundledSkillState, agents: &[PlacementAgent]) -> String {
    let labels: Vec<&str> = agents.iter().map(|agent| agent_label(*agent)).collect();
    let for_agents = if labels.is_empty() {
        String::new()
    } else {
        format!(" for {}", join_labels(&labels))
    };
    match state {
        BundledSkillState::NotPlaced => "Not installed".to_owned(),
        BundledSkillState::Current => format!("Installed{for_agents}"),
        BundledSkillState::Outdated => format!("Update available{for_agents}"),
        BundledSkillState::Unknown => "Status unknown".to_owned(),
    }
}

/// Install when not placed, Update when outdated and placed for an agent tool, Remove when placed for one.
fn action(state: BundledSkillState, agents: &[PlacementAgent]) -> Option<BundledAction> {
    match state {
        BundledSkillState::NotPlaced => Some(BundledAction::Install),
        BundledSkillState::Outdated if !agents.is_empty() => Some(BundledAction::Update),
        BundledSkillState::Current if !agents.is_empty() => Some(BundledAction::Remove),
        BundledSkillState::Outdated | BundledSkillState::Current | BundledSkillState::Unknown => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::base_bundled_skill;

    /// A bundled skill the service sends, with the name, state and agents the test varies.
    fn skill(name: &str, state: BundledSkillState, agents: &[PlacementAgent]) -> BundledSkill {
        BundledSkill {
            name: name.to_owned(),
            description: format!("Writes {name} notes"),
            state,
            agents: agents.to_vec(),
            ..base_bundled_skill()
        }
    }

    #[test]
    fn a_skill_that_is_not_placed_says_so_and_offers_install() {
        let rows = bundled_rows(&[skill("massorca", BundledSkillState::NotPlaced, &[])]);
        assert_eq!(rows[0].name, "massorca");
        assert_eq!(rows[0].description, "Writes massorca notes");
        assert_eq!(rows[0].state_line, "Not installed");
        assert_eq!(rows[0].action, Some(BundledAction::Install));
    }

    #[test]
    fn a_placed_skill_names_its_agent_tools_and_offers_remove() {
        let rows = bundled_rows(&[skill(
            "massorca",
            BundledSkillState::Current,
            &[PlacementAgent::Claude, PlacementAgent::Cursor],
        )]);
        assert_eq!(rows[0].state_line, "Installed for Claude Code and Cursor");
        assert_eq!(rows[0].action, Some(BundledAction::Remove));
        assert_eq!(
            rows[0].agents,
            [PlacementAgent::Claude, PlacementAgent::Cursor]
        );
    }

    #[test]
    fn an_outdated_skill_offers_update_and_one_with_no_agent_offers_nothing() {
        let rows = bundled_rows(&[
            skill(
                "massorca",
                BundledSkillState::Outdated,
                &[PlacementAgent::Codex],
            ),
            skill("notes", BundledSkillState::Outdated, &[]),
            skill("tools", BundledSkillState::Current, &[]),
        ]);
        assert_eq!(rows[0].state_line, "Update available for Codex");
        assert_eq!(rows[0].action, Some(BundledAction::Update));
        assert_eq!(rows[1].state_line, "Update available");
        assert_eq!(rows[1].action, None);
        assert_eq!(rows[2].state_line, "Installed");
        assert_eq!(rows[2].action, None);
    }

    #[test]
    fn a_state_from_a_newer_service_says_it_is_unknown_and_offers_nothing() {
        let rows = bundled_rows(&[skill(
            "massorca",
            BundledSkillState::Unknown,
            &[PlacementAgent::Claude],
        )]);
        assert_eq!(rows[0].state_line, "Status unknown");
        assert_eq!(rows[0].action, None);
    }

    #[test]
    fn the_review_names_the_agent_tools_and_an_install_with_no_folder_says_so() {
        assert_eq!(
            bundled_review_sentence(
                "massorca",
                false,
                &[PlacementAgent::Claude, PlacementAgent::Cursor]
            ),
            "Install massorca for Claude Code and Cursor?"
        );
        assert_eq!(
            bundled_review_sentence("massorca", true, &[PlacementAgent::Codex]),
            "Update massorca for Codex to this MassOrca version?"
        );
        assert_eq!(
            bundled_review_sentence("massorca", false, &[]),
            "There is no agent folder on this computer to install massorca in."
        );
    }
}
