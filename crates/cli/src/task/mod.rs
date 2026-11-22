//! `massorca task ...`: the tasks on a team's Board (decision 0072). `task start` is not built yet.

mod text;

use crate::target::Target;
use clap::{Args, Subcommand, ValueEnum};
use massorca_protocol::methods::tasks::{Column, TaskCreateParams, TaskListParams, TaskRef};

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use massorca_protocol::methods::tasks::TaskUpdateParams;

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// Creates a task on a team's Board, in the Open column.
    Create(TaskCreateArgs),
    /// Lists tasks, oldest first, filtered by team or Board column.
    List(TaskListArgs),
    /// Shows one task.
    Show {
        /// Task id.
        task: String,
    },
    /// Changes a task: its title, body, bot or project. An empty bot or project clears it.
    Update(TaskUpdateArgs),
    /// Starts the task's bot on it, in the team's folder workspace. A failed start keeps the task Open with the reason.
    Start {
        /// Task id.
        task: String,
    },
    /// Marks a task done by hand, for work that finished outside an agent.
    Done {
        /// Task id.
        task: String,
    },
    /// Clears a task's outcome and error. Refused while its agent session runs.
    Reopen {
        /// Task id.
        task: String,
    },
    /// Removes a task. Refused while its agent session runs.
    Rm {
        /// Task id.
        task: String,
    },
}

#[derive(Debug, Args)]
pub struct TaskCreateArgs {
    /// Team id or name.
    pub team: String,
    /// What the task is.
    pub title: String,
    /// Bot of the team that owns the task: id or name.
    #[arg(long, value_name = "BOT")]
    pub bot: Option<String>,
    /// Project of the team the task belongs to: id or name.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// More detail for the agent.
    #[arg(long, value_name = "TEXT")]
    pub body: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Only the tasks of this team: id or name.
    #[arg(long, value_name = "TEAM")]
    pub team: Option<String>,
    /// Only the tasks in this Board column.
    #[arg(long, value_enum, value_name = "COLUMN")]
    pub column: Option<ColumnArg>,
}

/// The Board columns a task list can be filtered by.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColumnArg {
    Open,
    Running,
    Waiting,
    Done,
}

impl From<ColumnArg> for Column {
    fn from(column: ColumnArg) -> Self {
        match column {
            ColumnArg::Open => Self::Open,
            ColumnArg::Running => Self::Running,
            ColumnArg::Waiting => Self::Waiting,
            ColumnArg::Done => Self::Done,
        }
    }
}

#[derive(Debug, Args)]
pub struct TaskUpdateArgs {
    /// Task id.
    pub task: String,
    /// New title of the task.
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,
    /// New body of the task. An empty value clears it.
    #[arg(long, value_name = "TEXT")]
    pub body: Option<String>,
    /// Bot of the team that owns the task: id or name. An empty value clears it.
    #[arg(long, value_name = "BOT")]
    pub bot: Option<String>,
    /// Project of the team the task belongs to: id or name. An empty value clears it.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
}

/// Runs one `task` subcommand against the service of `profile`.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: TaskCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    match command {
        TaskCommand::Create(args) => {
            let task = client
                .task_create(&TaskCreateParams {
                    team: args.team,
                    title: args.title,
                    body: args.body,
                    bot: args.bot,
                    project: args.project,
                    origin: None,
                })
                .await?;
            output.success(&task, &[text::task_line(&task, "created")]);
        }
        TaskCommand::List(args) => {
            let list = client
                .task_list(&TaskListParams {
                    team: args.team,
                    project: None,
                    bot: None,
                    column: args.column.map(Column::from),
                })
                .await?;
            let lines = if list.tasks.is_empty() {
                vec!["no tasks".to_owned()]
            } else {
                text::task_table(&list.tasks)
            };
            output.success(&list, &lines);
        }
        TaskCommand::Show { task } => {
            let shown = client.task_show(&TaskRef { task }).await?;
            output.success(&shown, &text::task_details(&shown));
        }
        TaskCommand::Update(args) => {
            let task = client
                .task_update(&TaskUpdateParams {
                    task: args.task,
                    title: args.title,
                    body: args.body,
                    bot: args.bot,
                    project: args.project,
                })
                .await?;
            output.success(&task, &[text::task_line(&task, "updated")]);
        }
        TaskCommand::Start { task } => {
            let started = client.task_start(&TaskRef { task }).await?;
            output.success(&started, &[text::task_line(&started, "started")]);
        }
        TaskCommand::Done { task } => {
            let done = client.task_done(&TaskRef { task }).await?;
            output.success(&done, &[text::task_line(&done, "done")]);
        }
        TaskCommand::Reopen { task } => {
            let reopened = client.task_reopen(&TaskRef { task }).await?;
            output.success(&reopened, &[text::task_line(&reopened, "reopened")]);
        }
        TaskCommand::Rm { task } => {
            let removed = client.task_remove(&TaskRef { task }).await?;
            output.success(&removed, &[text::task_line(&removed, "removed")]);
        }
    }
    Ok(())
}
