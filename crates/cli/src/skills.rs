//! `massorca skills installed` and `massorca skills roots` (SKL-01): the skills the agent tools and the registered projects
//! keep on this host, and the roots they were read from. Read only; the service does the scan.

use clap::Subcommand;
use massorca_protocol::methods::skills::{
    DiscoveredSkill, SkillSource, SkillsDiscoverParams, SkipReason,
};
use serde::Serialize;

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Lists the skills the agent tools and the registered projects keep on this host.
    Installed,
    /// Lists the skill roots this host reads, and the roots it skipped and why.
    Roots,
}

/// The `skills installed` result.
#[derive(Serialize)]
struct Installed<'a> {
    skills: &'a [DiscoveredSkill],
}

/// The `skills roots` result.
#[derive(Serialize)]
struct Roots<'a> {
    sources: &'a [SkillSource],
}

/// Runs one `skills` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: SkillCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let discovery = client
        .skills_discover(&SkillsDiscoverParams::default())
        .await?;
    match command {
        SkillCommand::Installed => output.success(
            &Installed {
                skills: &discovery.skills,
            },
            &skill_lines(&discovery.skills),
        ),
        SkillCommand::Roots => output.success(
            &Roots {
                sources: &discovery.sources,
            },
            &root_lines(&discovery.sources),
        ),
    }
    Ok(())
}

fn skill_lines(skills: &[DiscoveredSkill]) -> Vec<String> {
    if skills.is_empty() {
        return vec!["no skills found".to_owned()];
    }
    skills
        .iter()
        .map(|skill| match &skill.description {
            Some(description) => format!("{}  {}  {description}", skill.name, skill.source_label),
            None => format!("{}  {}", skill.name, skill.source_label),
        })
        .collect()
}

fn root_lines(sources: &[SkillSource]) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            let state = match (source.exists, source.skipped_reason) {
                (_, Some(SkipReason::Missing)) | (false, None) => "missing",
                (_, Some(SkipReason::Unreadable)) => "unreadable",
                (_, Some(SkipReason::Truncated)) => "truncated",
                (_, Some(SkipReason::Unknown)) => "skipped",
                (true, None) => "read",
            };
            format!("{}  {state}", source.label)
        })
        .collect()
}
