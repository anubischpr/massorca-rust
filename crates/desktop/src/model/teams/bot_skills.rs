//! A bot form's Skills chips (D-AG-33): the skills it keeps, and those the host discovered that it can still add.

use massorca_protocol::methods::skills::SkillDiscovery;
use massorca_protocol::methods::teams::MAX_BOT_SKILLS;

/// Why the Skills picker offers nothing to add.
pub const NO_SKILLS_FOUND: &str = "No skills found on this host. Add them in Marketplace > Skills.";
/// Why the Skills picker is off once a bot keeps the most skills it may.
pub const SKILLS_FULL: &str = "A bot keeps at most 50 skills.";

/// The names of the skills a host discovered, each once, sorted.
pub fn discovered_skill_names(discovery: &SkillDiscovery) -> Vec<String> {
    let mut names: Vec<String> = discovery
        .skills
        .iter()
        .map(|skill| skill.name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

/// The discovered skills the bot does not keep yet, in the order given.
pub fn skills_to_add(discovered: &[String], chosen: &[String]) -> Vec<String> {
    discovered
        .iter()
        .filter(|name| !chosen.contains(name))
        .cloned()
        .collect()
}

/// `chosen` with `skill` added at the end; a repeat, a blank, or a full list changes nothing.
pub fn with_skill(chosen: &[String], skill: &str) -> Vec<String> {
    let mut kept = chosen.to_vec();
    let skill = skill.trim();
    if !skill.is_empty() && !kept.iter().any(|name| name == skill) && kept.len() < MAX_BOT_SKILLS {
        kept.push(skill.to_owned());
    }
    kept
}

/// `chosen` without `skill`.
pub fn without_skill(chosen: &[String], skill: &str) -> Vec<String> {
    chosen
        .iter()
        .filter(|name| *name != skill)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::skills::{DiscoveredSkill, SkillSourceKind};

    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn skill(name: &str) -> DiscoveredSkill {
        DiscoveredSkill {
            id: name.to_owned(),
            name: name.to_owned(),
            description: None,
            providers: Vec::new(),
            source_kind: SkillSourceKind::Home,
            source_label: "Home".to_owned(),
            root_path: None,
            root_paths: Vec::new(),
            directory_path: None,
            skill_file_path: None,
            installed: true,
            updated_at: None,
            disable_slash_command: false,
        }
    }

    #[test]
    fn discovered_names_are_sorted_once_without_blanks() {
        let discovery = SkillDiscovery {
            skills: vec![
                skill("review"),
                skill("Deploy"),
                skill(" "),
                skill("review"),
            ],
            sources: Vec::new(),
            scanned_at: 0,
        };
        assert_eq!(
            discovered_skill_names(&discovery),
            names(&["Deploy", "review"])
        );
    }

    #[test]
    fn the_picker_offers_only_skills_the_bot_does_not_keep() {
        let discovered = names(&["deploy", "notes", "review"]);
        assert_eq!(
            skills_to_add(&discovered, &names(&["review"])),
            names(&["deploy", "notes"])
        );
    }

    #[test]
    fn a_skill_is_added_once_at_the_end_and_removed_by_name() {
        let chosen = with_skill(&names(&["review"]), " deploy ");
        assert_eq!(chosen, names(&["review", "deploy"]));
        assert_eq!(with_skill(&chosen, "review"), chosen);
        assert_eq!(without_skill(&chosen, "review"), names(&["deploy"]));
    }

    #[test]
    fn a_full_list_takes_no_more_skills() {
        let full: Vec<String> = (0..MAX_BOT_SKILLS).map(|n| format!("skill-{n}")).collect();
        assert_eq!(with_skill(&full, "one-more"), full);
    }
}
