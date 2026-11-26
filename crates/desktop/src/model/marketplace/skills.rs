//! The skills agent tools keep on this host, as the Skills section lists them (SKL-01): one row per skill,
//! however many folders hold it, and each root with why it was or was not read.

use massorca_protocol::methods::skills::{
    DiscoveredSkill, SkillDiscovery, SkillProvider, SkillSource, SkillSourceKind, SkipReason,
};

/// One skill as a row of the Skills section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// The agents whose folders hold the skill, as their labels, in the order the service lists them.
    pub agents: Vec<&'static str>,
    /// The label of the root the skill was read from.
    pub source: String,
    /// Whether a copy of the skill is installed in an agent folder.
    pub installed: bool,
}

/// One root a scan read or skipped, as the per-root report lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootRow {
    pub label: String,
    /// The agents that read the root, as their labels.
    pub agents: Vec<&'static str>,
    /// Home, Workspace, Bundled or Plugin; None for a kind a newer service sends.
    pub kind: Option<&'static str>,
    pub exists: bool,
    /// Why the root was not read in full; None when it was read.
    pub skip_reason: Option<&'static str>,
}

/// The rows of a discovery, sorted by name ignoring case, then by id.
pub fn skill_rows(discovery: &SkillDiscovery) -> Vec<SkillRow> {
    let mut rows: Vec<SkillRow> = discovery.skills.iter().map(skill_row).collect();
    rows.sort_by_cached_key(|row| (row.name.to_lowercase(), row.id.clone()));
    rows
}

fn skill_row(skill: &DiscoveredSkill) -> SkillRow {
    SkillRow {
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        agents: agent_labels(&skill.providers),
        source: skill.source_label.clone(),
        installed: skill.installed,
    }
}

/// The roots of a discovery, in the order the service lists them.
pub fn root_rows(discovery: &SkillDiscovery) -> Vec<RootRow> {
    discovery.sources.iter().map(root_row).collect()
}

fn root_row(source: &SkillSource) -> RootRow {
    RootRow {
        label: source.label.clone(),
        agents: agent_labels(&source.providers),
        kind: kind_label(source.source_kind),
        exists: source.exists,
        skip_reason: source.skipped_reason.map(skip_label),
    }
}

/// The rows whose name or description contains the query, ignoring case and surrounding spaces.
/// An empty query keeps every row.
pub fn search_skills<'a>(rows: &'a [SkillRow], query: &str) -> Vec<&'a SkillRow> {
    let query = query.trim().to_lowercase();
    rows.iter()
        .filter(|row| {
            query.is_empty()
                || row.name.to_lowercase().contains(&query)
                || row
                    .description
                    .as_deref()
                    .is_some_and(|text| text.to_lowercase().contains(&query))
        })
        .collect()
}

/// "1 skill", or "3 skills".
pub fn skill_count_line(count: usize) -> String {
    if count == 1 {
        "1 skill".to_owned()
    } else {
        format!("{count} skills")
    }
}

/// The labels of the agents this build knows, each once, in the order the service lists them.
fn agent_labels(providers: &[SkillProvider]) -> Vec<&'static str> {
    let mut labels: Vec<&'static str> = Vec::new();
    for provider in providers {
        let label = match provider {
            SkillProvider::Codex => "Codex",
            SkillProvider::Claude => "Claude",
            SkillProvider::AgentSkills => "Agent Skills",
            SkillProvider::Unknown => continue,
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels
}

fn kind_label(kind: SkillSourceKind) -> Option<&'static str> {
    match kind {
        SkillSourceKind::Home => Some("Home"),
        SkillSourceKind::Repo => Some("Workspace"),
        SkillSourceKind::Bundled => Some("Bundled"),
        SkillSourceKind::Plugin => Some("Plugin"),
        SkillSourceKind::Unknown => None,
    }
}

/// The words pv96's Skills page uses for a root it did not read; a truncated root has no pv96 words yet.
fn skip_label(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::Missing => "Folder not found",
        SkipReason::Truncated => "Scan stopped early",
        SkipReason::Unreadable | SkipReason::Unknown => "Not scanned",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(
        id: &str,
        name: &str,
        description: Option<&str>,
        providers: &[SkillProvider],
        source: &str,
    ) -> DiscoveredSkill {
        DiscoveredSkill {
            id: id.to_owned(),
            name: name.to_owned(),
            description: description.map(str::to_owned),
            providers: providers.to_vec(),
            source_label: source.to_owned(),
            ..crate::service::fixtures::base_discovered_skill()
        }
    }

    fn root(
        label: &str,
        kind: SkillSourceKind,
        providers: &[SkillProvider],
        exists: bool,
        skipped: Option<SkipReason>,
    ) -> SkillSource {
        SkillSource {
            id: label.to_owned(),
            label: label.to_owned(),
            source_kind: kind,
            providers: providers.to_vec(),
            exists,
            skipped_reason: skipped,
            ..crate::service::fixtures::base_skill_source()
        }
    }

    fn discovery(skills: Vec<DiscoveredSkill>, sources: Vec<SkillSource>) -> SkillDiscovery {
        SkillDiscovery {
            skills,
            sources,
            ..crate::service::fixtures::base_skill_discovery()
        }
    }

    #[test]
    fn skills_are_listed_by_name_ignoring_case_with_their_agents_and_source() {
        let found = discovery(
            vec![
                skill(
                    "b",
                    "release-notes",
                    Some("Writes notes"),
                    &[SkillProvider::Claude, SkillProvider::Codex],
                    "Claude home",
                ),
                skill(
                    "a",
                    "Changelog",
                    None,
                    &[SkillProvider::AgentSkills],
                    "Shared folder",
                ),
            ],
            Vec::new(),
        );
        let rows = skill_rows(&found);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["Changelog", "release-notes"]);
        assert_eq!(rows[0].agents, ["Agent Skills"]);
        assert_eq!(rows[1].agents, ["Claude", "Codex"]);
        assert_eq!(rows[1].source, "Claude home");
        assert_eq!(rows[1].description.as_deref(), Some("Writes notes"));
        assert_eq!(rows[0].description, None);
        assert!(rows[0].installed);
    }

    #[test]
    fn a_provider_or_kind_from_a_newer_service_is_left_out_rather_than_shown_raw() {
        let found = discovery(
            vec![skill(
                "a",
                "x",
                None,
                &[SkillProvider::Unknown, SkillProvider::Codex],
                "Home",
            )],
            vec![root(
                "Home",
                SkillSourceKind::Unknown,
                &[SkillProvider::Unknown],
                true,
                None,
            )],
        );
        assert_eq!(skill_rows(&found)[0].agents, ["Codex"]);
        let roots = root_rows(&found);
        assert_eq!(roots[0].kind, None);
        assert!(roots[0].agents.is_empty());
    }

    #[test]
    fn each_root_reports_its_agents_kind_existence_and_why_it_was_not_read() {
        let found = discovery(
            Vec::new(),
            vec![
                root(
                    "Claude home",
                    SkillSourceKind::Home,
                    &[SkillProvider::Claude],
                    true,
                    None,
                ),
                root(
                    "Project",
                    SkillSourceKind::Repo,
                    &[SkillProvider::AgentSkills],
                    true,
                    Some(SkipReason::Truncated),
                ),
                root(
                    "Codex home",
                    SkillSourceKind::Home,
                    &[SkillProvider::Codex],
                    false,
                    Some(SkipReason::Missing),
                ),
                root(
                    "Plugin cache",
                    SkillSourceKind::Plugin,
                    &[],
                    true,
                    Some(SkipReason::Unreadable),
                ),
            ],
        );
        let roots = root_rows(&found);
        assert_eq!(
            roots[0],
            RootRow {
                label: "Claude home".to_owned(),
                agents: vec!["Claude"],
                kind: Some("Home"),
                exists: true,
                skip_reason: None,
            }
        );
        assert_eq!(roots[1].kind, Some("Workspace"));
        assert_eq!(roots[1].skip_reason, Some("Scan stopped early"));
        assert!(!roots[2].exists);
        assert_eq!(roots[2].skip_reason, Some("Folder not found"));
        assert_eq!(roots[3].kind, Some("Plugin"));
        assert_eq!(roots[3].skip_reason, Some("Not scanned"));
    }

    #[test]
    fn the_search_matches_names_and_descriptions_ignoring_case_and_empty_keeps_every_row() {
        let found = discovery(
            vec![
                skill("a", "Changelog", None, &[], "Home"),
                skill("b", "notes", Some("Writes a CHANGELOG entry"), &[], "Home"),
            ],
            Vec::new(),
        );
        let rows = skill_rows(&found);
        assert_eq!(search_skills(&rows, "  changelog ").len(), 2);
        assert_eq!(search_skills(&rows, "").len(), 2);
        assert_eq!(search_skills(&rows, "entry")[0].name, "notes");
        assert!(search_skills(&rows, "missing").is_empty());
    }

    #[test]
    fn the_count_line_says_skill_or_skills() {
        assert_eq!(skill_count_line(0), "0 skills");
        assert_eq!(skill_count_line(1), "1 skill");
        assert_eq!(skill_count_line(3), "3 skills");
    }
}
