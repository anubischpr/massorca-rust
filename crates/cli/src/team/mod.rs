//! `massorca team ...`: teams, their bots and projects, and their Boards (decision 0072).

mod text;

use crate::target::Target;
use clap::{Args, Subcommand, ValueEnum};
use massorca_client::Client;
use massorca_protocol::methods::teams::{
    BoardParams, BotAddParams, BotRemoveParams, TeamCharacter, TeamCreateParams,
    TeamProjectAddParams, TeamRef, TeamRemoveParams,
};

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    /// Creates a team. Its name is unique.
    Create(CreateArgs),
    /// Lists every team, ordered by name.
    List,
    /// Shows one team, selected by id or name.
    Show {
        /// Team id or name.
        team: String,
    },
    /// Removes a team with its bots, projects and tasks. Refused while a task runs, unless --force.
    Rm {
        /// Team id or name.
        team: String,
        /// Remove the team even while one of its tasks runs.
        #[arg(long)]
        force: bool,
    },
    /// Adds and removes a team's bots.
    Bot {
        #[command(subcommand)]
        command: BotCommand,
    },
    /// Adds a team's projects.
    Project {
        #[command(subcommand)]
        command: TeamProjectCommand,
    },
    /// Shows a team's Board: its tasks in four columns, or only one project's.
    Board {
        /// Team id or name.
        team: String,
        /// Only this project's tasks (id or name within the team).
        #[arg(long, value_name = "PROJECT")]
        project: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Name of the team.
    pub name: String,
    /// What the team is for.
    #[arg(long, value_name = "TEXT")]
    pub purpose: Option<String>,
    /// The team's shape. Defaults to the triangle.
    #[arg(long, value_enum, value_name = "SHAPE")]
    pub character: Option<CharacterArg>,
}

/// The shapes a team can take.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CharacterArg {
    Triangle,
    Square,
    Circle,
    Diamond,
    Hexagon,
    Star,
}

impl From<CharacterArg> for TeamCharacter {
    fn from(shape: CharacterArg) -> Self {
        match shape {
            CharacterArg::Triangle => Self::Triangle,
            CharacterArg::Square => Self::Square,
            CharacterArg::Circle => Self::Circle,
            CharacterArg::Diamond => Self::Diamond,
            CharacterArg::Hexagon => Self::Hexagon,
            CharacterArg::Star => Self::Star,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum BotCommand {
    /// Adds a bot to a team. Its agent kind and job are required.
    Add(BotAddArgs),
    /// Removes a bot. Refused while it owns open tasks, unless --unassign is given.
    Rm {
        /// Team id or name.
        team: String,
        /// Bot id or name within the team.
        bot: String,
        /// Remove the bot and leave its open tasks without a bot.
        #[arg(long)]
        unassign: bool,
    },
}

#[derive(Debug, Args)]
pub struct BotAddArgs {
    /// Team id or name.
    pub team: String,
    /// Name of the bot, unique within the team.
    pub name: String,
    /// The agent kind the bot runs as, such as claude.
    #[arg(long, value_name = "KIND")]
    pub agent: String,
    /// What the bot is good at.
    #[arg(long, value_name = "TEXT")]
    pub job: String,
    /// Make the bot the team's lead. Refused while another bot leads.
    #[arg(long)]
    pub lead: bool,
}

#[derive(Debug, Subcommand)]
pub enum TeamProjectCommand {
    /// Adds a project to a team. Its name is unique within the team.
    Add(TeamProjectAddArgs),
}

#[derive(Debug, Args)]
pub struct TeamProjectAddArgs {
    /// Team id or name.
    pub team: String,
    /// Name of the project, unique within the team.
    pub name: String,
    /// What the project is for.
    #[arg(long, value_name = "TEXT")]
    pub goal: Option<String>,
}

/// Runs one `team` subcommand against the service of `profile`.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: TeamCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    match command {
        TeamCommand::Create(args) => {
            let team = client.team_create(&create_params(args)).await?;
            output.success(&team, &[text::team_line(&team, "created")]);
        }
        TeamCommand::List => {
            let list = client.team_list().await?;
            let lines = if list.teams.is_empty() {
                vec!["no teams".to_owned()]
            } else {
                text::team_table(&list.teams)
            };
            output.success(&list, &lines);
        }
        TeamCommand::Show { team } => {
            let shown = client.team_show(&TeamRef { team }).await?;
            output.success(&shown, &text::team_details(&shown));
        }
        TeamCommand::Rm { team, force } => {
            let removed = client
                .team_remove(&TeamRemoveParams { team, force })
                .await?;
            output.success(&removed, &[text::team_line(&removed, "removed")]);
        }
        TeamCommand::Bot { command } => run_bot(&client, command, output).await?,
        TeamCommand::Project { command } => run_project(&client, command, output).await?,
        TeamCommand::Board { team, project } => {
            let board = client.team_board(&BoardParams { team, project }).await?;
            output.success(&board, &text::board_lines(&board));
        }
    }
    Ok(())
}

async fn run_bot(client: &Client, command: BotCommand, output: Output) -> Result<(), CliError> {
    match command {
        BotCommand::Add(args) => {
            let bot = client
                .team_bot_add(&BotAddParams {
                    team: args.team,
                    name: args.name,
                    job: args.job,
                    kind: args.agent,
                    model: None,
                    effort: None,
                    lead: args.lead,
                })
                .await?;
            output.success(&bot, &[text::bot_line(&bot, "added")]);
        }
        BotCommand::Rm {
            team,
            bot,
            unassign,
        } => {
            let removed = client
                .team_bot_remove(&BotRemoveParams {
                    team,
                    bot,
                    unassign,
                })
                .await?;
            output.success(&removed, &[text::bot_line(&removed, "removed")]);
        }
    }
    Ok(())
}

async fn run_project(
    client: &Client,
    command: TeamProjectCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        TeamProjectCommand::Add(args) => {
            let project = client
                .team_project_add(&TeamProjectAddParams {
                    team: args.team,
                    name: args.name,
                    goal: args.goal,
                    brief: None,
                })
                .await?;
            output.success(&project, &[text::project_line(&project, "added")]);
        }
    }
    Ok(())
}

fn create_params(args: CreateArgs) -> TeamCreateParams {
    TeamCreateParams {
        name: args.name,
        purpose: args.purpose,
        character: args.character.map(TeamCharacter::from),
        access: None,
        workspaces: None,
    }
}
