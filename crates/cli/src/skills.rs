//! `massorca skills`: `installed` and `roots` (SKL-01) list what the agent tools and the registered projects keep on this
//! host; `preview`, `place`, `unplace` and `managed` (SKL-02) copy a skill into the agents' folders and remove only the
//! copies MassOrca placed. The service does the scans and the writes. `bundled` (SKL-04) lists, installs, refreshes and removes
//! the skills MassOrca ships; `bundled show` prints one the way this build renders it and needs no service.

use clap::Subcommand;
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::skill_bundled::{
    BundledSkill, BundledSkillState, SkillsBundledInstallParams, SkillsBundledNameParams,
};
use massorca_protocol::methods::skill_placement::{
    PlacementAgent, PlacementState, PlacementTopology, SkillPlaceParams, SkillPlacementResult,
    SkillUnplaceParams, SkillsManagedParams, SkillsManagedResult,
};
use massorca_protocol::methods::skills::{
    DiscoveredSkill, SkillSource, SkillsDiscoverParams, SkipReason,
};
use serde::Serialize;

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::skill_guide::{SKILL_NAME, massorca_skill_text};
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
    /// Lists the skills MassOrca placed on this host.
    Managed,
    /// The skills MassOrca ships: list them, install, refresh or remove one, or show its text (SKL-04).
    #[command(subcommand)]
    Bundled(BundledCommand),
}

#[derive(Debug, Subcommand)]
pub enum BundledCommand {
    /// Lists the skills MassOrca ships, and whether each is placed on this host.
    List,
    /// Places a skill MassOrca ships for the chosen agents; without --agent, for every agent folder on this host.
    Install(BundledInstallArgs),
    /// Brings a placed skill MassOrca ships to this build's text. A copy the person changed is kept.
    Refresh(BundledNameArgs),
    /// Removes a skill MassOrca ships from the agent folders it was placed in. A copy the person changed is kept.
    Remove(BundledNameArgs),
    /// Prints a bundled skill's text as this build renders it. The checked-in copy must equal it; no service is needed.
    Show(BundledNameArgs),
}

#[derive(Debug, clap::Args)]
pub struct BundledNameArgs {
    /// The bundled skill's name, as `skills bundled list` shows it.
    pub name: String,
}

#[derive(Debug, clap::Args)]
pub struct BundledInstallArgs {
    /// The bundled skill's name, as `skills bundled list` shows it.
    pub name: String,
    /// An agent to place the skill for; repeat for several. Without it, every agent folder on this host.
    #[arg(long = "agent", value_enum)]
    pub agents: Vec<AgentArg>,
}

#[derive(Debug, clap::Args)]
pub struct PlaceArgs {
    /// The id `skills installed` shows for the skill.
    pub skill_id: String,
    /// An agent to place the skill for; repeat for several.
    #[arg(long = "agent", value_enum, required = true)]
    pub agents: Vec<AgentArg>,
}

#[derive(Debug, clap::Args)]
pub struct UnplaceArgs {
    /// The skill's name, as `skills managed` shows it.
    pub name: String,
    /// Remove only this agent's copy; repeat for several. Without it, every copy MassOrca placed goes.
    #[arg(long = "agent", value_enum)]
    pub agents: Vec<AgentArg>,
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
    // Why: `bundled show` reads only this build, so it must not start or reach the service.
    if let SkillCommand::Bundled(BundledCommand::Show(args)) = &command {
        return show_bundled(&args.name, output);
    }
    let client = connect(target, autostart).await?;
    run_service_command(&client, command, output).await
}

async fn run_service_command(
    client: &Client,
    command: SkillCommand,
    output: Output,
) -> Result<(), CliError> {
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
        SkillCommand::Bundled(command) => run_bundled(client, command, output).await?,
    }
    Ok(())
}

/// Runs one `skills bundled` subcommand. `show` is answered here without the service; the others go to it.
async fn run_bundled(
    client: &Client,
    command: BundledCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        BundledCommand::List => {
            let listed = client.skills_bundled_list().await?;
            output.success(&listed, &bundled_lines(&listed.skills));
        }
        BundledCommand::Install(args) => {
            let result = client
                .skills_bundled_install(&SkillsBundledInstallParams {
                    name: args.name,
                    agents: wire_agents(&args.agents),
                })
                .await?;
            output.success(&result, &placement_lines(&result));
        }
        BundledCommand::Refresh(args) => {
            let result = client
                .skills_bundled_refresh(&SkillsBundledNameParams { name: args.name })
                .await?;
            output.success(&result, &placement_lines(&result));
        }
        BundledCommand::Remove(args) => {
            let result = client
                .skills_unplace(&SkillUnplaceParams {
                    name: args.name,
                    agents: Vec::new(),
                })
                .await?;
            output.success(&result, &placement_lines(&result));
        }
        BundledCommand::Show(args) => show_bundled(&args.name, output)?,
    }
    Ok(())
}

/// One line per bundled skill: its name, its state on this host, and the agents it is placed for.
fn bundled_lines(skills: &[BundledSkill]) -> Vec<String> {
    if skills.is_empty() {
        return vec!["MassOrca ships no skills".to_owned()];
    }
    skills
        .iter()
        .map(|skill| {
            let state = match skill.state {
                BundledSkillState::NotPlaced => "not installed",
                BundledSkillState::Current => "installed",
                BundledSkillState::Outdated => "update available",
                BundledSkillState::Unknown => "status unknown",
            };
            let agents: Vec<&str> = skill
                .agents
                .iter()
                .map(|agent| agent_name(*agent))
                .collect();
            if agents.is_empty() {
                format!("{}  {state}", skill.name)
            } else {
                format!("{}  {state} for {}", skill.name, agents.join(", "))
            }
        })
        .collect()
}

/// The `bundled show` result: the skill's name and its text.
#[derive(Serialize)]
struct BundledText<'a> {
    name: &'a str,
    text: &'a str,
}

/// Prints the bundled skill's text as this build renders it: exactly the checked-in file, so the file can be regenerated
/// from this command. Only the MassOrca skill is bundled.
fn show_bundled(name: &str, output: Output) -> Result<(), CliError> {
    if name != SKILL_NAME {
        return Err(CliError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("no bundled skill is named {name:?}"),
            exit::USAGE,
        ));
    }
    let text = massorca_skill_text();
    output.success(
        &BundledText { name, text: &text },
        &text.lines().map(str::to_owned).collect::<Vec<_>>(),
    );
    Ok(())
}

fn place_params(args: PlaceArgs) -> SkillPlaceParams {
    SkillPlaceParams {
        skill_id: args.skill_id,
        agents: wire_agents(&args.agents),
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
