//! `massorca skills`: `installed` and `roots` (SKL-01) list what the agent tools and the registered projects keep on this
//! host; `preview`, `place`, `unplace` and `managed` (SKL-02) copy a skill into the agents' folders and remove only the
//! copies MassOrca placed. The service does the scans and the writes.

use clap::Subcommand;
use massorca_protocol::methods::skill_placement::{
    PlacementAgent, PlacementScope, PlacementState, PlacementTarget, PlacementTopology,
    SkillAgentsParams, SkillPlaceParams, SkillPlacementResult, SkillUnplaceParams,
    SkillsManagedParams, SkillsManagedResult,
};
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
    /// Shows what placing a skill for agent tools would do. Changes nothing.
    Preview(PlaceArgs),
    /// Copies a skill the agent tools keep into the chosen agents' folders, and records the copies.
    Place(PlaceArgs),
    /// Removes the copies MassOrca placed for a skill. A copy the user changed is kept.
    Unplace(UnplaceArgs),
    /// Adds agents to, or drops agents from, a placed skill in one scope; the agents not named stay as they are.
    Agents(AgentsArgs),
    /// Lists the skills MassOrca placed on this host.
    Managed,
}

#[derive(Debug, clap::Args)]
pub struct PlaceArgs {
    /// The id `skills installed` shows for the skill.
    pub skill_id: String,
    /// An agent to place the skill for; repeat for several.
    #[arg(long = "agent", value_enum, required = true)]
    pub agents: Vec<AgentArg>,
    /// Place the skill in this MassOrca workspace (its id) instead of the home folder.
    #[arg(long = "workspace")]
    pub workspace: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct UnplaceArgs {
    /// The skill's name, as `skills managed` shows it.
    pub name: String,
    /// Remove only this agent's copy; repeat for several. Without it, every copy MassOrca placed goes.
    #[arg(long = "agent", value_enum)]
    pub agents: Vec<AgentArg>,
    /// Remove the copies of this MassOrca workspace (its id) instead of the home folder's.
    #[arg(long = "workspace")]
    pub workspace: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct AgentsArgs {
    /// The skill's name, as `skills managed` shows it.
    pub name: String,
    /// An agent to add to the placed skill, which gets its own folder; repeat for several.
    #[arg(long = "add", value_enum)]
    pub add: Vec<AgentArg>,
    /// An agent to drop from the placed skill, which loses its own copy; repeat for several.
    #[arg(long = "drop", value_enum)]
    pub drop: Vec<AgentArg>,
    /// The MassOrca workspace (its id) whose copies change; without it, the home folder's.
    #[arg(long = "workspace")]
    pub workspace: Option<String>,
}

/// An agent tool a skill can be placed for.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum AgentArg {
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
    match command {
        SkillCommand::Installed => {
            let discovery = client
                .skills_discover(&SkillsDiscoverParams::default())
                .await?;
            output.success(
                &Installed {
                    skills: &discovery.skills,
                },
                &skill_lines(&discovery.skills),
            );
        }
        SkillCommand::Roots => {
            let discovery = client
                .skills_discover(&SkillsDiscoverParams::default())
                .await?;
            output.success(
                &Roots {
                    sources: &discovery.sources,
                },
                &root_lines(&discovery.sources),
            );
        }
        SkillCommand::Preview(args) => {
            let result = client.skills_place_preview(&place_params(args)).await?;
            output.success(&result, &placement_lines(&result));
        }
        SkillCommand::Place(args) => {
            let result = client.skills_place(&place_params(args)).await?;
            output.success(&result, &placement_lines(&result));
        }
        SkillCommand::Unplace(args) => {
            let result = client
                .skills_unplace(&SkillUnplaceParams {
                    name: args.name,
                    agents: wire_agents(&args.agents),
                    target: target_of(args.workspace),
                })
                .await?;
            output.success(&result, &placement_lines(&result));
        }
        SkillCommand::Agents(args) => {
            let result = client
                .skills_agents(&SkillAgentsParams {
                    name: args.name,
                    add: wire_agents(&args.add),
                    drop: wire_agents(&args.drop),
                    target: target_of(args.workspace),
                })
                .await?;
            output.success(&result, &placement_lines(&result));
        }
        SkillCommand::Managed => {
            let managed = client
                .skills_managed(&SkillsManagedParams::default())
                .await?;
            output.success(&managed, &managed_lines(&managed));
        }
    }
    Ok(())
}

fn place_params(args: PlaceArgs) -> SkillPlaceParams {
    SkillPlaceParams {
        skill_id: args.skill_id,
        agents: wire_agents(&args.agents),
        target: target_of(args.workspace),
    }
}

/// The scope a `--workspace` id names: the workspace's folder, else the home folder.
fn target_of(workspace: Option<String>) -> PlacementTarget {
    match workspace {
        Some(id) => PlacementTarget {
            scope: PlacementScope::Workspace,
            workspace_id: Some(id),
        },
        None => PlacementTarget::default(),
    }
}

fn wire_agents(agents: &[AgentArg]) -> Vec<PlacementAgent> {
    agents
        .iter()
        .map(|agent| match agent {
            AgentArg::Codex => PlacementAgent::Codex,
            AgentArg::Claude => PlacementAgent::Claude,
            AgentArg::Cursor => PlacementAgent::Cursor,
            AgentArg::Gemini => PlacementAgent::Gemini,
            AgentArg::Droid => PlacementAgent::Droid,
            AgentArg::Continue => PlacementAgent::Continue,
            AgentArg::Trae => PlacementAgent::Trae,
            AgentArg::Grok => PlacementAgent::Grok,
            AgentArg::Aug => PlacementAgent::Aug,
            AgentArg::Muse => PlacementAgent::Muse,
            AgentArg::Zcode => PlacementAgent::Zcode,
        })
        .collect()
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

/// The skill's name, then one line per agent with what happened for it.
fn placement_lines(result: &SkillPlacementResult) -> Vec<String> {
    let mut lines = vec![result.name.clone()];
    lines.extend(result.agents.iter().map(|row| {
        let path = row
            .path
            .as_deref()
            .map(|path| format!("  {path}"))
            .unwrap_or_default();
        let via = match row.topology {
            Some(PlacementTopology::Alias) => " via alias",
            Some(PlacementTopology::Copy) => " via copy",
            _ => "",
        };
        format!(
            "  {}  {}{via}{path}",
            agent_name(row.agent),
            state_text(row.state)
        )
    }));
    lines
}

fn managed_lines(managed: &SkillsManagedResult) -> Vec<String> {
    if managed.skills.is_empty() {
        return vec!["no skills placed by MassOrca".to_owned()];
    }
    managed
        .skills
        .iter()
        .map(|skill| {
            let agents: Vec<&str> = skill
                .agents
                .iter()
                .map(|row| agent_name(row.agent))
                .collect();
            if agents.is_empty() {
                format!("{}  canonical copy only", skill.name)
            } else {
                format!("{}  {}", skill.name, agents.join(", "))
            }
        })
        .collect()
}

fn agent_name(agent: PlacementAgent) -> &'static str {
    match agent {
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
        PlacementAgent::Unknown => "unknown agent",
    }
}

fn state_text(state: PlacementState) -> &'static str {
    match state {
        PlacementState::Canonical => "reads the canonical copy",
        PlacementState::Create => "would place",
        PlacementState::Placed => "placed",
        PlacementState::AlreadyPlaced => "already placed",
        PlacementState::Removed => "removed",
        PlacementState::SkippedExisting => "skipped: a folder you own is there",
        PlacementState::SkippedModified => "skipped: changed since MassOrca placed it",
        PlacementState::SkippedCopyFailed => "skipped: the copy failed",
        PlacementState::Unknown => "unknown state",
    }
}
