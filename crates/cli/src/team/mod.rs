//! `massorca team ...`: teams, their bots, projects, memory and routines, and their Boards (decision 0072).

mod routine;
mod text;

mod delegate;
use delegate::DelegateArgs;
use routine::RoutineCommand;

use crate::agent_hook::{ENV_SESSION, ENV_TOKEN};
use crate::target::Target;
use clap::{Args, Subcommand, ValueEnum};
use massorca_client::Client;
use massorca_protocol::methods::team_room::{
    TeamRoomAnswerParams, TeamRoomListParams, TeamRoomMentionsParams, TeamRoomPostParams,
    TeamRoomResolveParams,
};
use massorca_protocol::methods::teams::{
    Board, BoardParams, BotAddParams, BotRemoveParams, CodeSource, TeamAccess, TeamCharacter,
    TeamCreateParams, TeamProjectAddParams, TeamRef, TeamRemoveParams,
};
use std::collections::HashSet;

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use massorca_protocol::methods::teams::{
    BotUpdateParams, TeamMemoryAddParams, TeamMemoryListParams, TeamMemoryRemoveParams,
    TeamProjectRemoveParams, TeamProjectShowParams, TeamProjectUpdateParams, TeamUpdateParams,
};
use massorca_protocol::{capabilities, codes};

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
    /// Changes a team: its name, purpose, shape or workspaces flag. Only the options given change.
    Update(TeamUpdateArgs),
    /// Lists, adds, changes and removes a team's bots.
    Bot {
        #[command(subcommand)]
        command: BotCommand,
    },
    /// Lists, adds, changes and removes a team's projects.
    Project {
        #[command(subcommand)]
        command: TeamProjectCommand,
    },
    /// Lists, adds and removes a team's memory lines, in the team scope or in one bot's.
    Memory {
        #[command(subcommand)]
        command: TeamMemoryCommand,
    },
    /// Reads and posts to a team's Conference room. A person's line becomes a task for a bot, which starts at once.
    Room {
        #[command(subcommand)]
        command: TeamRoomCommand,
    },
    /// Hands a task to a teammate from inside a bot's agent run: the child starts in its own workspace (decision 0128).
    Delegate(DelegateArgs),
    /// Lists, adds, changes, pauses, runs and removes a team's routines: schedules that start a bot on a task.
    Routine {
        #[command(subcommand)]
        command: RoutineCommand,
    },
    /// Shows a team's Board: its tasks in four columns, or only one project's, or only the tasks
    /// no project holds.
    Board {
        /// Team id or name.
        team: String,
        /// Only this project's tasks (id or name within the team).
        #[arg(long, value_name = "PROJECT", conflicts_with = "no_project")]
        project: Option<String>,
        /// Only the tasks no project of the team holds.
        #[arg(long, conflicts_with = "project")]
        no_project: bool,
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
    /// How the team's bots run: full (without asking for permission, the default) or default (the agent's own mode).
    #[arg(long, value_enum, value_name = "ACCESS")]
    pub access: Option<AccessArg>,
}

/// How a team's bots run (decision 0072, amendment "team access").
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AccessArg {
    /// Bots run in their agent's bypass mode, without asking for permission.
    Full,
    /// Bots run in their agent's own default mode, which may ask for permission.
    Default,
}

impl From<AccessArg> for TeamAccess {
    fn from(access: AccessArg) -> Self {
        match access {
            AccessArg::Full => Self::Full,
            AccessArg::Default => Self::Default,
        }
    }
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
    /// Lists a team's bots, in the order they were added.
    List {
        /// Team id or name.
        team: String,
    },
    /// Changes a bot. Only the options given change; an empty model or effort clears it.
    Update(BotUpdateArgs),
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
    /// The bot's skills, by name, separated by commas (at most 50).
    #[arg(long, value_name = "SKILLS")]
    pub skills: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum TeamProjectCommand {
    /// Lists a team's projects, in the order they were added.
    List {
        /// Team id or name.
        team: String,
    },
    /// Shows one project: its goal, its code source and the Workspaces project it links.
    Show {
        /// Team id or name.
        team: String,
        /// Project id or name within the team.
        project: String,
    },
    /// Changes a team's project. Only the options given change.
    Update(TeamProjectUpdateArgs),
    /// Removes a project. Its tasks stay on the Board without it.
    Rm {
        /// Team id or name.
        team: String,
        /// Project id or name within the team.
        project: String,
    },
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
    /// The Workspaces project (id, name or path) the code comes from. Needs Workspaces in the team.
    #[arg(long, value_name = "PROJECT")]
    pub source: Option<String>,
    /// Where the code is, recorded only (nothing is cloned or created). Needs Workspaces in the team.
    #[arg(long, value_enum, value_name = "KIND")]
    pub code_source: Option<CodeSourceArg>,
}

#[derive(Debug, Subcommand)]
pub enum TeamMemoryCommand {
    /// Lists a team's memory lines, newest first: all of the team's, or only one bot's with --bot.
    List {
        /// Team id or name.
        team: String,
        /// Only this bot's lines: id or name within the team.
        #[arg(long, value_name = "BOT")]
        bot: Option<String>,
    },
    /// Adds a line of at most 500 characters: to the team's memory, or to one bot's with --bot.
    Add(TeamMemoryAddArgs),
    /// Removes a memory line by its id.
    Rm {
        /// Team id or name.
        team: String,
        /// Line id.
        line: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TeamRoomCommand {
    /// Lists a team's room, oldest first: who wrote each line, its text, and the task it is about.
    List {
        /// Team id or name.
        team: String,
        /// Only the newest this many lines (200 without it).
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },
    /// Posts a line of at most 4000 characters. As a person, the line becomes a task for the bot it names with
    /// @name, else --to, else the lead, else the only bot, and the task starts unless --no-start. With several bots and
    /// none of those, the room answers with a choice card instead (see `team room answer`). The task's title is
    /// the line's first line without the @name of the team's bots, cut to 80 characters. A person's line that names one
    /// day (YYYY-MM-DD, today, tomorrow or a weekday) and asks for one time ("at 09:30", "at 9pm", "a las 17", or any
    /// time in a line that says schedule or remind) becomes that bot's one-time routine on the host's clock instead; a
    /// routine the host refuses leaves it a task. With --bot, that bot posts the line.
    /// Inside an agent run, the line is posted as the bot of the run's own task or reply, and naming another bot is
    /// refused. With --ask, or --reply-to a bot's line, the line is a conversation instead: one bot (the one it names,
    /// else the one it replies to, else --to, else the lead, else the first) answers it once in the room, in at most
    /// 1,200 characters, and no task is made. `team room list` shows who is typing.
    Post {
        /// Team id or name.
        team: String,
        /// The line.
        text: String,
        /// Post as this bot: id or name within the team.
        #[arg(long, value_name = "BOT")]
        bot: Option<String>,
        /// The bot a person's line goes to when it names none: id or name within the team.
        #[arg(long, value_name = "BOT", conflicts_with = "bot")]
        to: Option<String>,
        /// Leave a person's task in Todo instead of starting it.
        #[arg(long, conflicts_with = "bot")]
        no_start: bool,
        /// The line this one replies to: its id from `team room list --json`. A reply keeps the quoted line as its context.
        #[arg(long, value_name = "MESSAGE", conflicts_with = "bot")]
        reply_to: Option<String>,
        /// Ask a bot without making a task.
        #[arg(long, conflicts_with_all = ["bot", "task", "no_start"])]
        ask: bool,
        /// Make a reply to a bot's line a task instead of a conversation.
        #[arg(long, conflicts_with = "bot", requires = "reply_to")]
        task: bool,
    },
    /// Answers a choice card once: `bot:<id>` gives the request to that bot, `team:<id>` puts it in another team's
    /// Todo, and `todo` keeps it in this team's Todo with no bot. `team room list` prints each card's choices.
    Answer {
        /// Team id or name.
        team: String,
        /// The card's line id.
        message: String,
        /// One of the card's choices.
        choice: String,
    },
    /// Lists what an @ typed so far can name in the room: the team's bots (@Name), the other teams (@team:Name), the
    /// team's projects (@project:Name) and @file:. A QUERY starting with file: lists the files and folders of one folder
    /// of the team's projects (file:src/ lists src). Needs a host with team-room/4.
    Mentions {
        /// Team id or name.
        team: String,
        /// The text after the @ typed so far.
        #[arg(default_value = "")]
        query: String,
        /// At most this many (20 without it, 100 at most).
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },
    /// Shows what each mention of TEXT names, as a posted line reads it: @Name a bot of the team, @team:Name another
    /// team, @project:Name a project of the team, @file:path a file inside a project's folder. A typed mention that
    /// names nothing is refused (team.room_mention_unknown), and so is a file outside its project
    /// (team.room_file_outside).
    Resolve {
        /// Team id or name.
        team: String,
        /// The line.
        text: String,
    },
}

#[derive(Debug, Args)]
pub struct TeamMemoryAddArgs {
    /// Team id or name.
    pub team: String,
    /// The line, at most 500 characters.
    pub text: String,
    /// The bot whose memory gets the line: id or name within the team.
    #[arg(long, value_name = "BOT")]
    pub bot: Option<String>,
    /// The task that wrote the line, by id; the line is then a task line.
    #[arg(long, value_name = "TASK")]
    pub task: Option<String>,
}

#[derive(Debug, Args)]
pub struct TeamUpdateArgs {
    /// Team id or name.
    pub team: String,
    /// New name of the team.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// New purpose. An empty value clears it.
    #[arg(long, value_name = "TEXT")]
    pub purpose: Option<String>,
    /// New shape of the team.
    #[arg(long, value_enum, value_name = "SHAPE")]
    pub character: Option<CharacterArg>,
    /// Whether the team has a folder for Workspaces: true or false.
    #[arg(long, value_name = "BOOL", value_parser = clap::value_parser!(bool))]
    pub workspaces: Option<bool>,
    /// How the team's bots run: full (without asking for permission) or default (the agent's own mode).
    #[arg(long, value_enum, value_name = "ACCESS")]
    pub access: Option<AccessArg>,
}

#[derive(Debug, Args)]
pub struct BotUpdateArgs {
    /// Team id or name.
    pub team: String,
    /// Bot id or name within the team.
    pub bot: String,
    /// New name of the bot.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// New job of the bot.
    #[arg(long, value_name = "TEXT")]
    pub job: Option<String>,
    /// The agent kind the bot runs as, such as claude.
    #[arg(long, value_name = "KIND")]
    pub agent: Option<String>,
    /// The model the bot runs with. An empty value clears it.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
    /// The effort the bot runs with. An empty value clears it.
    #[arg(long, value_name = "EFFORT")]
    pub effort: Option<String>,
    /// Make the bot the lead (true), or clear its lead (false). Refused while another bot leads.
    #[arg(long, value_name = "BOOL", value_parser = clap::value_parser!(bool))]
    pub lead: Option<bool>,
    /// Replaces the bot's skills with these names, separated by commas. An empty value clears them.
    #[arg(long, value_name = "SKILLS")]
    pub skills: Option<String>,
}

#[derive(Debug, Args)]
pub struct TeamProjectUpdateArgs {
    /// Team id or name.
    pub team: String,
    /// Project id or name within the team.
    pub project: String,
    /// New name of the project.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// New goal of the project.
    #[arg(long, value_name = "TEXT")]
    pub goal: Option<String>,
    /// New Brief of the project. An empty value clears it.
    #[arg(long, value_name = "TEXT")]
    pub brief: Option<String>,
    /// The Workspaces project (id, name or path) the code comes from; linking needs Workspaces in the team. An empty
    /// value clears it.
    #[arg(long, value_name = "PROJECT")]
    pub source: Option<String>,
    /// Where the code is, recorded only (nothing is cloned or created). `none` clears it; a kind needs Workspaces.
    #[arg(long, value_enum, value_name = "KIND")]
    pub code_source: Option<CodeSourceArg>,
}

/// Where a team project's code is (pv96's "Existing repository" or "New folder with git").
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CodeSourceArg {
    /// An existing repository.
    ExistingRepository,
    /// A new folder with git.
    NewFolderWithGit,
    /// No code source. In an update it clears the recorded one.
    None,
}

impl From<CodeSourceArg> for CodeSource {
    fn from(source: CodeSourceArg) -> Self {
        match source {
            CodeSourceArg::ExistingRepository => Self::ExistingRepository,
            CodeSourceArg::NewFolderWithGit => Self::NewFolderWithGit,
            CodeSourceArg::None => Self::None,
        }
    }
}

/// Runs one `team` subcommand against the service of `profile`.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: TeamCommand,
    output: Output,
) -> Result<(), CliError> {
    // Checked before connecting: outside an agent run there is no service call to make.
    if let TeamCommand::Delegate(args) = command {
        return delegate::run(target, autostart, args, output).await;
    }
    let client = connect(target, autostart).await?;
    match command {
        TeamCommand::Create(args) => {
            let team = client.team_create(&create_params(args)).await?;
            let mut lines = vec![text::team_line(&team, "created")];
            lines.extend(text::access_notice(&team));
            output.success(&team, &lines);
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
        TeamCommand::Update(args) => {
            let team = client
                .team_update(&TeamUpdateParams {
                    team: args.team,
                    name: args.name,
                    purpose: args.purpose,
                    character: args.character.map(TeamCharacter::from),
                    access: args.access.map(TeamAccess::from),
                    workspaces: args.workspaces,
                })
                .await?;
            let mut lines = vec![text::team_line(&team, "updated")];
            lines.extend(text::access_notice(&team));
            output.success(&team, &lines);
        }
        TeamCommand::Bot { command } => run_bot(&client, command, output).await?,
        TeamCommand::Project { command } => run_project(&client, command, output).await?,
        TeamCommand::Memory { command } => run_memory(&client, command, output).await?,
        TeamCommand::Room { command } => run_room(&client, command, output).await?,
        TeamCommand::Routine { command } => routine::run(&client, command, output).await?,
        TeamCommand::Delegate(_) => unreachable!("team delegate is handled before connecting"),
        TeamCommand::Board {
            team,
            project,
            no_project,
        } => {
            let board = if no_project {
                board_without_projects(&client, team).await?
            } else {
                client.team_board(&BoardParams { team, project }).await?
            };
            output.success(&board, &text::board_lines(&board));
        }
    }
    Ok(())
}

/// The team's Board without the cards that any of its projects holds: the tasks no project holds (`--no-project`).
async fn board_without_projects(client: &Client, team: String) -> Result<Board, CliError> {
    let mut board = client
        .team_board(&BoardParams {
            team: team.clone(),
            project: None,
        })
        .await?;
    let mut held = HashSet::new();
    for project in client
        .team_project_list(&TeamRef { team: team.clone() })
        .await?
        .projects
    {
        let project_board = client
            .team_board(&BoardParams {
                team: team.clone(),
                project: Some(project.id),
            })
            .await?;
        held.extend(
            project_board
                .columns
                .into_iter()
                .flat_map(|column| column.cards)
                .map(|card| card.id),
        );
    }
    for column in &mut board.columns {
        column.cards.retain(|card| !held.contains(&card.id));
    }
    Ok(board)
}

/// The names of a `--skills` value; blanks are dropped, so an empty value is an empty list.
fn skill_names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

async fn run_bot(client: &Client, command: BotCommand, output: Output) -> Result<(), CliError> {
    match command {
        BotCommand::List { team } => {
            let list = client.team_bot_list(&TeamRef { team }).await?;
            let lines = if list.bots.is_empty() {
                vec!["no bots".to_owned()]
            } else {
                text::bot_table(&list.bots)
            };
            output.success(&list, &lines);
        }
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
                    skills: args.skills.as_deref().map(skill_names).unwrap_or_default(),
                })
                .await?;
            output.success(&bot, &[text::bot_line(&bot, "added")]);
        }
        BotCommand::Update(args) => {
            let bot = client
                .team_bot_update(&BotUpdateParams {
                    team: args.team,
                    bot: args.bot,
                    name: args.name,
                    job: args.job,
                    kind: args.agent,
                    model: args.model,
                    effort: args.effort,
                    lead: args.lead,
                    skills: args.skills.as_deref().map(skill_names),
                })
                .await?;
            output.success(&bot, &[text::bot_line(&bot, "updated")]);
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
        TeamProjectCommand::List { team } => {
            let list = client.team_project_list(&TeamRef { team }).await?;
            let lines = if list.projects.is_empty() {
                vec!["no projects".to_owned()]
            } else {
                text::project_table(&list.projects)
            };
            output.success(&list, &lines);
        }
        TeamProjectCommand::Show { team, project } => {
            let shown = client
                .team_project_show(&TeamProjectShowParams { team, project })
                .await?;
            output.success(&shown, &text::project_details(&shown));
        }
        TeamProjectCommand::Update(args) => {
            let project = client
                .team_project_update(&TeamProjectUpdateParams {
                    team: args.team,
                    project: args.project,
                    name: args.name,
                    goal: args.goal,
                    brief: args.brief,
                    source_project: args.source,
                    code_source: args.code_source.map(CodeSource::from),
                })
                .await?;
            output.success(&project, &[text::project_line(&project, "updated")]);
        }
        TeamProjectCommand::Rm { team, project } => {
            let removed = client
                .team_project_remove(&TeamProjectRemoveParams { team, project })
                .await?;
            output.success(&removed, &[text::project_line(&removed, "removed")]);
        }
        TeamProjectCommand::Add(args) => {
            let project = client
                .team_project_add(&TeamProjectAddParams {
                    team: args.team,
                    name: args.name,
                    goal: args.goal,
                    brief: None,
                    source_project: args.source,
                    code_source: args.code_source.map(CodeSource::from),
                })
                .await?;
            output.success(&project, &[text::project_line(&project, "added")]);
        }
    }
    Ok(())
}

async fn run_memory(
    client: &Client,
    command: TeamMemoryCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        TeamMemoryCommand::List { team, bot } => {
            let list = client
                .team_memory_list(&TeamMemoryListParams { team, bot })
                .await?;
            let lines = if list.lines.is_empty() {
                vec!["no memory".to_owned()]
            } else {
                text::memory_table(&list.lines)
            };
            output.success(&list, &lines);
        }
        TeamMemoryCommand::Add(args) => {
            let line = client
                .team_memory_add(&TeamMemoryAddParams {
                    team: args.team,
                    bot: args.bot,
                    text: args.text,
                    task: args.task,
                })
                .await?;
            output.success(&line, &[text::memory_line(&line, "added")]);
        }
        TeamMemoryCommand::Rm { team, line } => {
            let removed = client
                .team_memory_remove(&TeamMemoryRemoveParams { team, line })
                .await?;
            output.success(&removed, &[text::memory_line(&removed, "removed")]);
        }
    }
    Ok(())
}

async fn run_room(
    client: &Client,
    command: TeamRoomCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        TeamRoomCommand::List { team, limit } => {
            let room = client
                .team_room_list(&TeamRoomListParams { team, limit })
                .await?;
            let mut lines = if room.messages.is_empty() {
                vec!["no lines".to_owned()]
            } else {
                text::room_lines(&room.messages)
            };
            lines.extend(text::typing_line(&room.typing));
            output.success(&room, &lines);
        }
        TeamRoomCommand::Post {
            team,
            text,
            bot,
            to,
            no_start,
            reply_to,
            ask,
            task,
        } => {
            // An older host ignores both and makes a task, so they need a host that answers in the room.
            if (ask || reply_to.is_some())
                && !client.has_capability(capabilities::TEAM_ROOM_REPLIES)
            {
                return Err(CliError::new(
                    codes::CAPABILITY_NOT_NEGOTIATED,
                    format!(
                        "this service does not offer {}, so it cannot answer in the room; update MassOrca on the host",
                        capabilities::TEAM_ROOM_REPLIES
                    ),
                    exit::ERROR,
                ));
            }
            // Inside an agent run, the run's session and token bind the line to the bot of that run's task.
            let (session, agent_token) = agent_run();
            let posted = client
                .team_room_post(&TeamRoomPostParams {
                    team,
                    text,
                    bot,
                    to,
                    session,
                    agent_token,
                    start: no_start.then_some(false),
                    reply_to,
                    ask: if ask {
                        Some(true)
                    } else {
                        task.then_some(false)
                    },
                    route: None,
                })
                .await?;
            output.success(&posted, &text::room_lines(&posted.messages));
        }
        TeamRoomCommand::Answer {
            team,
            message,
            choice,
        } => {
            let answered = client
                .team_room_answer(&TeamRoomAnswerParams {
                    team,
                    message,
                    choice,
                })
                .await?;
            let mut lines = text::room_lines(std::slice::from_ref(&answered.card));
            lines.extend(text::room_lines(&answered.messages));
            output.success(&answered, &lines);
        }
        TeamRoomCommand::Mentions { team, query, limit } => {
            let mentions = client
                .team_room_mentions(&TeamRoomMentionsParams { team, query, limit })
                .await?;
            output.success(&mentions, &text::mention_lines(&mentions.mentions));
        }
        TeamRoomCommand::Resolve { team, text } => {
            let mentions = client
                .team_room_resolve(&TeamRoomResolveParams { team, text })
                .await?;
            output.success(&mentions, &text::mention_lines(&mentions.mentions));
        }
    }
    Ok(())
}

/// The session and run token of the agent run this command runs in, when both are set.
fn agent_run() -> (Option<String>, Option<String>) {
    let read = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    match (read(ENV_SESSION), read(ENV_TOKEN)) {
        (Some(session), Some(token)) => (Some(session), Some(token)),
        _ => (None, None),
    }
}

fn create_params(args: CreateArgs) -> TeamCreateParams {
    TeamCreateParams {
        name: args.name,
        purpose: args.purpose,
        character: args.character.map(TeamCharacter::from),
        access: args.access.map(TeamAccess::from),
        workspaces: None,
    }
}
