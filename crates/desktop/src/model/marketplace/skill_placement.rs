//! The words and choices of placing a skill for agent tools (SKL-02): which agents a review names, the one sentence a
//! review shows, what a finished placement says, and the copies MassOrca placed.

use massorca_protocol::methods::skill_placement::{
    ManagedSkill, PlacementAgent, PlacementState, SkillPlacementResult,
};
use massorca_protocol::methods::skills::SkillSource;

/// The agent tool's name as the app shows it.
pub fn agent_label(agent: PlacementAgent) -> &'static str {
    match agent {
        PlacementAgent::Codex => "Codex",
        PlacementAgent::Claude => "Claude Code",
        PlacementAgent::Cursor => "Cursor",
        PlacementAgent::Gemini => "Gemini CLI",
        PlacementAgent::Droid => "Droid",
        PlacementAgent::Continue => "Continue",
        PlacementAgent::Trae => "Trae",
        PlacementAgent::Grok => "Grok",
        PlacementAgent::Aug => "Augment",
        PlacementAgent::Muse => "Muse",
        PlacementAgent::Zcode => "ZCode",
        PlacementAgent::Unknown => "Unknown agent",
    }
}

/// The agent a discovery root's owner names, or `None` for an owner this build cannot place for.
pub fn placement_agent_for_owner(owner: &str) -> Option<PlacementAgent> {
    Some(match owner {
        "codex" => PlacementAgent::Codex,
        "claude" => PlacementAgent::Claude,
        "cursor" => PlacementAgent::Cursor,
        "gemini" => PlacementAgent::Gemini,
        "droid" => PlacementAgent::Droid,
        "continue" => PlacementAgent::Continue,
        "trae" => PlacementAgent::Trae,
        "grok" => PlacementAgent::Grok,
        "aug" => PlacementAgent::Aug,
        "muse" => PlacementAgent::Muse,
        "zcode" => PlacementAgent::Zcode,
        _ => return None,
    })
}

/// The agents whose folder exists on this computer, in the order the discovery lists their roots. These are the agents a
/// placement offers; an agent whose folder is missing is not installed here, so nothing is created for it.
pub fn detected_agents(sources: &[SkillSource]) -> Vec<PlacementAgent> {
    let mut agents = Vec::new();
    for source in sources.iter().filter(|source| source.exists) {
        let Some(agent) = source.owner.as_deref().and_then(placement_agent_for_owner) else {
            continue;
        };
        if !agents.contains(&agent) {
            agents.push(agent);
        }
    }
    agents
}

/// "Cursor", "Cursor and Gemini CLI", or "Codex, Cursor and Gemini CLI".
pub(super) fn join_labels(labels: &[&str]) -> String {
    match labels {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}

/// The one sentence a review shows before a placement: the skill and the agent tools it goes to.
pub fn place_review_sentence(name: &str, agents: &[PlacementAgent]) -> String {
    if agents.is_empty() {
        return format!("{name} is already placed for every agent tool it can be placed for.");
    }
    let labels: Vec<&str> = agents.iter().map(|agent| agent_label(*agent)).collect();
    format!("Place {name} for {}?", join_labels(&labels))
}

/// The one sentence a review shows before a removal: the copies MassOrca placed go, and a changed copy stays.
pub fn remove_review_sentence(name: &str) -> String {
    format!("Remove the copies MassOrca placed for {name}? A copy you changed is kept.")
}

/// The folders a placement would create, one line each as "Cursor: /home/you/.cursor/skills/notes". The paths are host
/// paths, so they belong under "Show details".
pub fn plan_detail_lines(result: &SkillPlacementResult) -> Vec<String> {
    result
        .agents
        .iter()
        .filter(|row| row.state == PlacementState::Create)
        .filter_map(|row| {
            row.path
                .as_ref()
                .map(|path| format!("{}: {path}", agent_label(row.agent)))
        })
        .collect()
}

/// What a finished placement or removal says, in one sentence.
pub fn result_sentence(result: &SkillPlacementResult, removing: bool) -> String {
    let done = if removing {
        PlacementState::Removed
    } else {
        PlacementState::Placed
    };
    let labels: Vec<&str> = result
        .agents
        .iter()
        .filter(|row| row.state == done)
        .map(|row| agent_label(row.agent))
        .collect();
    let kept = result
        .agents
        .iter()
        .filter(|row| {
            matches!(
                row.state,
                PlacementState::SkippedExisting
                    | PlacementState::SkippedModified
                    | PlacementState::SkippedCopyFailed
            )
        })
        .count();
    let mut sentence = match (removing, labels.is_empty()) {
        (true, true) => format!("Nothing was removed for {}.", result.name),
        (true, false) => format!("Removed {} for {}.", result.name, join_labels(&labels)),
        (false, true) => format!("Nothing was placed for {}.", result.name),
        (false, false) => format!("Placed {} for {}.", result.name, join_labels(&labels)),
    };
    if kept > 0 {
        sentence.push_str(&format!(" {kept} folder(s) left as they were."));
    }
    sentence
}

/// One managed skill as a row of the placed list: its name and the agent tools it is placed for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedRow {
    pub name: String,
    pub agents: String,
}

/// The placed skills as rows, by name.
pub fn managed_rows(skills: &[ManagedSkill]) -> Vec<ManagedRow> {
    let mut rows: Vec<ManagedRow> = skills
        .iter()
        .map(|skill| {
            let labels: Vec<&str> = skill
                .agents
                .iter()
                .filter(|row| row.path.is_some())
                .map(|row| agent_label(row.agent))
                .collect();
            let agents = if labels.is_empty() {
                "canonical copy only".to_owned()
            } else {
                join_labels(&labels)
            };
            ManagedRow {
                name: skill.name.clone(),
                agents,
            }
        })
        .collect();
    rows.sort_by_key(|row| row.name.to_lowercase());
    rows
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::skill_placement::{AgentPlacement, ManagedAgent};
    use massorca_protocol::methods::skills::{SkillSource, SkillSourceKind};

    use super::*;

    fn row(agent: PlacementAgent, state: PlacementState, path: Option<&str>) -> AgentPlacement {
        AgentPlacement {
            agent,
            state,
            path: path.map(str::to_owned),
            canonical: false,
            topology: None,
        }
    }

    fn result(rows: Vec<AgentPlacement>) -> SkillPlacementResult {
        SkillPlacementResult {
            name: "notes".to_owned(),
            canonical_path: None,
            agents: rows,
        }
    }

    fn source(owner: Option<&str>, exists: bool) -> SkillSource {
        SkillSource {
            id: "id".to_owned(),
            label: "label".to_owned(),
            path: None,
            source_kind: SkillSourceKind::Home,
            providers: Vec::new(),
            owner: owner.map(str::to_owned),
            exists,
            skipped_reason: None,
        }
    }

    #[test]
    fn the_review_names_the_agents_in_one_plain_sentence() {
        assert_eq!(
            place_review_sentence("notes", &[PlacementAgent::Cursor]),
            "Place notes for Cursor?"
        );
        assert_eq!(
            place_review_sentence(
                "notes",
                &[
                    PlacementAgent::Codex,
                    PlacementAgent::Cursor,
                    PlacementAgent::Gemini
                ]
            ),
            "Place notes for Codex, Cursor and Gemini CLI?"
        );
        assert_eq!(
            place_review_sentence("notes", &[]),
            "notes is already placed for every agent tool it can be placed for."
        );
    }

    #[test]
    fn only_agents_whose_folder_exists_here_are_offered() {
        let sources = [
            source(Some("claude"), true),
            source(Some("cursor"), false),
            source(None, true),
            source(Some("unknown-tool"), true),
            source(Some("claude"), true),
        ];
        assert_eq!(detected_agents(&sources), [PlacementAgent::Claude]);
    }

    #[test]
    fn a_result_says_what_was_placed_and_how_many_folders_were_left_alone() {
        let placed = result(vec![
            row(PlacementAgent::Codex, PlacementState::Canonical, None),
            row(
                PlacementAgent::Cursor,
                PlacementState::Placed,
                Some("/h/.cursor"),
            ),
            row(
                PlacementAgent::Gemini,
                PlacementState::SkippedExisting,
                None,
            ),
        ]);
        assert_eq!(
            result_sentence(&placed, false),
            "Placed notes for Cursor. 1 folder(s) left as they were."
        );
    }

    #[test]
    fn a_removal_names_what_went_and_an_empty_one_says_so() {
        let removed = result(vec![row(
            PlacementAgent::Cursor,
            PlacementState::Removed,
            Some("/h/.cursor"),
        )]);
        assert_eq!(result_sentence(&removed, true), "Removed notes for Cursor.");
        assert_eq!(
            result_sentence(&result(Vec::new()), true),
            "Nothing was removed for notes."
        );
    }

    #[test]
    fn the_plan_lists_only_the_folders_it_would_create() {
        let plan = result(vec![
            row(
                PlacementAgent::Cursor,
                PlacementState::Create,
                Some("/h/.cursor/skills/notes"),
            ),
            row(
                PlacementAgent::Claude,
                PlacementState::SkippedExisting,
                Some("/h/.claude/skills/notes"),
            ),
        ]);
        assert_eq!(
            plan_detail_lines(&plan),
            ["Cursor: /h/.cursor/skills/notes"]
        );
    }

    #[test]
    fn managed_rows_name_each_placed_agent_and_sort_by_name() {
        let skills = [
            ManagedSkill {
                name: "Zeta".to_owned(),
                canonical_path: None,
                agents: vec![ManagedAgent {
                    agent: PlacementAgent::Cursor,
                    path: Some("/h/.cursor/skills/zeta".to_owned()),
                    canonical: false,
                    topology: None,
                }],
            },
            ManagedSkill {
                name: "alpha".to_owned(),
                canonical_path: None,
                agents: Vec::new(),
            },
        ];
        let rows = managed_rows(&skills);
        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[0].agents, "canonical copy only");
        assert_eq!(rows[1].agents, "Cursor");
    }

    #[test]
    fn owners_map_to_their_agents_and_unknown_owners_do_not() {
        assert_eq!(
            placement_agent_for_owner("gemini"),
            Some(PlacementAgent::Gemini)
        );
        assert_eq!(placement_agent_for_owner("opencode"), None);
    }
}
