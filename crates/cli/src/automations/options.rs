//! The options of `massorca automations create` and `edit` (decision 0123), and their wire values.

use clap::{Args, ValueEnum};
use massorca_protocol::methods::automations::{AutomationCadence, AutomationWeekday};

/// How often an automation runs.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CadenceArg {
    Hourly,
    Daily,
    Weekdays,
    Weekly,
    Cron,
}

impl From<CadenceArg> for AutomationCadence {
    fn from(cadence: CadenceArg) -> Self {
        match cadence {
            CadenceArg::Hourly => Self::Hourly,
            CadenceArg::Daily => Self::Daily,
            CadenceArg::Weekdays => Self::Weekdays,
            CadenceArg::Weekly => Self::Weekly,
            CadenceArg::Cron => Self::Cron,
        }
    }
}

/// A day of the week, for a weekly automation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WeekdayArg {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl From<WeekdayArg> for AutomationWeekday {
    fn from(day: WeekdayArg) -> Self {
        match day {
            WeekdayArg::Mon => Self::Monday,
            WeekdayArg::Tue => Self::Tuesday,
            WeekdayArg::Wed => Self::Wednesday,
            WeekdayArg::Thu => Self::Thursday,
            WeekdayArg::Fri => Self::Friday,
            WeekdayArg::Sat => Self::Saturday,
            WeekdayArg::Sun => Self::Sunday,
        }
    }
}

/// Options of `automations create`.
#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Name of the automation.
    pub name: String,
    /// The project the runs work in: its id, its name or its path. Without it, the project of `--workspace`, or of the
    /// MassOrca worktree the current directory is in.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// The prompt the agent starts with on each run.
    #[arg(long, value_name = "TEXT")]
    pub prompt: String,
    /// The agent kind to run, such as `claude`.
    #[arg(long, value_name = "KIND", default_value = "claude")]
    pub agent: String,
    /// How often the automation runs.
    #[arg(long, value_enum, default_value = "daily")]
    pub cadence: CadenceArg,
    /// The 24-hour time, HH:MM. For an hourly automation, its minute. Not used by a cron automation.
    #[arg(long, value_name = "HH:MM", default_value = "09:00")]
    pub time: String,
    /// The day of a weekly automation.
    #[arg(long, value_enum, value_name = "DAY")]
    pub weekday: Option<WeekdayArg>,
    /// The five fields of a cron automation: minute, hour, day of month, month, day of week.
    #[arg(long, value_name = "EXPRESSION")]
    pub cron: Option<String>,
    /// An IANA time zone, such as Europe/Madrid. Without it the host's zone is used.
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
    /// Minutes a missed run may be late and still run, 0 to 10080. The default is 720.
    #[arg(long, value_name = "MINUTES")]
    pub grace: Option<i64>,
    /// A key a retried create repeats, so the retry returns the automation the first create made.
    #[arg(long, value_name = "KEY")]
    pub creation_key: Option<String>,
    /// Runs in this existing workspace, by id, a worktree or a folder: every run goes there.
    #[arg(long, value_name = "WORKSPACE", conflicts_with = "new_worktree")]
    pub workspace: Option<String>,
    /// Starts a new worktree for each run, titled with the automation's name, in the project's repository.
    #[arg(long, conflicts_with = "workspace")]
    pub new_worktree: bool,
    /// The branch a new worktree per run starts from; needs `--new-worktree`.
    #[arg(long, value_name = "BRANCH", requires = "new_worktree")]
    pub base_branch: Option<String>,
}

/// Options of `automations edit`. Only the options given change; `--zone ""` returns it to the host's zone.
#[derive(Debug, Args)]
pub struct EditArgs {
    /// The automation, by id or exact name.
    pub automation: String,
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    #[arg(long, value_name = "TEXT")]
    pub prompt: Option<String>,
    #[arg(long, value_name = "KIND")]
    pub agent: Option<String>,
    #[arg(long, value_enum)]
    pub cadence: Option<CadenceArg>,
    #[arg(long, value_name = "HH:MM")]
    pub time: Option<String>,
    #[arg(long, value_enum, value_name = "DAY")]
    pub weekday: Option<WeekdayArg>,
    #[arg(long, value_name = "EXPRESSION")]
    pub cron: Option<String>,
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
    #[arg(long, value_name = "MINUTES")]
    pub grace: Option<i64>,
    /// Runs in this existing workspace, by id, a worktree or a folder: every run goes there.
    #[arg(long, value_name = "WORKSPACE", conflicts_with = "new_worktree")]
    pub workspace: Option<String>,
    /// Starts a new worktree for each run, titled with the automation's name, in the project's repository.
    #[arg(long, conflicts_with = "workspace")]
    pub new_worktree: bool,
    /// The branch a new worktree per run starts from; needs `--new-worktree`.
    #[arg(long, value_name = "BRANCH", requires = "new_worktree")]
    pub base_branch: Option<String>,
}
