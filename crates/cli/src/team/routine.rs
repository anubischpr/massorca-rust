//! `massorca team routine ...`: a team's routines, which start a bot on a task each time they are due (decision 0072
//! amendment, decision 0082). `run --now` makes one run for testing; the schedule runs routines on its own.

use clap::{Args, Subcommand, ValueEnum};
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::teams::{
    RoutineAddParams, RoutineCadence, RoutineRef, RoutineRunsParams, RoutineUpdateParams,
    RoutineWeekday, TeamRef,
};

use super::text;
use crate::error::{CliError, exit};
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum RoutineCommand {
    /// Lists a team's routines, in creation order.
    List {
        /// Team id or name.
        team: String,
    },
    /// Adds a routine: a bot does a task at a time of day, daily, weekly, monthly or once.
    Add(RoutineAddArgs),
    /// Changes a routine. Only the options given change; `--zone ""` returns it to the host's zone.
    Update(RoutineUpdateArgs),
    /// Removes a routine and its runs. The tasks the runs made stay on the Board.
    Rm {
        /// Team id or name.
        team: String,
        /// Routine id or name within the team.
        routine: String,
    },
    /// Pauses a routine: it runs no more until it is resumed.
    Pause {
        /// Team id or name.
        team: String,
        /// Routine id or name within the team.
        routine: String,
    },
    /// Resumes a paused routine from now. Runs missed while it was paused are not made up.
    Resume {
        /// Team id or name.
        team: String,
        /// Routine id or name within the team.
        routine: String,
    },
    /// Lists a routine's runs, newest first.
    Runs {
        /// Team id or name.
        team: String,
        /// Routine id or name within the team.
        routine: String,
        /// The most runs to show; 50 when unset.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
    /// Makes one run now, for testing. The routine's schedule does not move; `--now` is required.
    Run {
        /// Team id or name.
        team: String,
        /// Routine id or name within the team.
        routine: String,
        /// Confirms the run now.
        #[arg(long)]
        now: bool,
    },
}

#[derive(Debug, Args)]
pub struct RoutineAddArgs {
    /// Team id or name.
    pub team: String,
    /// Name of the routine; each task it makes takes this title.
    pub name: String,
    /// The bot that does the work: id or name within the team.
    #[arg(long, value_name = "BOT")]
    pub bot: String,
    /// The prompt, which is the body of each task the routine makes.
    #[arg(long, value_name = "TEXT")]
    pub prompt: String,
    /// The time of day, 24-hour HH:MM.
    #[arg(long, value_name = "HH:MM")]
    pub time: String,
    /// How often the routine runs.
    #[arg(long, value_enum)]
    pub cadence: CadenceArg,
    /// The day of a weekly routine.
    #[arg(long, value_enum, value_name = "DAY")]
    pub weekday: Option<WeekdayArg>,
    /// The day of the month, 1 to 28, of a monthly routine.
    #[arg(long, value_name = "DAY")]
    pub month_day: Option<u8>,
    /// The date of a once routine, YYYY-MM-DD.
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub date: Option<String>,
    /// An IANA time zone, such as Europe/Madrid. Without it the host's zone is used.
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
}

#[derive(Debug, Args)]
pub struct RoutineUpdateArgs {
    /// Team id or name.
    pub team: String,
    /// Routine id or name within the team.
    pub routine: String,
    /// The bot that does the work: id or name within the team.
    #[arg(long, value_name = "BOT")]
    pub bot: Option<String>,
    /// New name of the routine.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// New prompt, the body of each task the routine makes.
    #[arg(long, value_name = "TEXT")]
    pub prompt: Option<String>,
    /// How often the routine runs.
    #[arg(long, value_enum)]
    pub cadence: Option<CadenceArg>,
    /// New time of day, 24-hour HH:MM.
    #[arg(long, value_name = "HH:MM")]
    pub time: Option<String>,
    /// The day of a weekly routine.
    #[arg(long, value_enum, value_name = "DAY")]
    pub weekday: Option<WeekdayArg>,
    /// The day of the month, 1 to 28, of a monthly routine.
    #[arg(long, value_name = "DAY")]
    pub month_day: Option<u8>,
    /// The date of a once routine, YYYY-MM-DD.
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub date: Option<String>,
    /// An IANA time zone; an empty value returns the routine to the host's zone.
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
}

/// How often a routine runs.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CadenceArg {
    Daily,
    Weekly,
    Monthly,
    Once,
}

/// A day of the week.
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

impl From<CadenceArg> for RoutineCadence {
    fn from(cadence: CadenceArg) -> Self {
        match cadence {
            CadenceArg::Daily => Self::Daily,
            CadenceArg::Weekly => Self::Weekly,
            CadenceArg::Monthly => Self::Monthly,
            CadenceArg::Once => Self::Once,
        }
    }
}

impl From<WeekdayArg> for RoutineWeekday {
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

/// Runs one `team routine` subcommand.
pub async fn run(client: &Client, command: RoutineCommand, output: Output) -> Result<(), CliError> {
    match command {
        RoutineCommand::List { team } => {
            let list = client
                .team_routine_list(&TeamRef { team })
                .await
                .map_err(|error| too_old(error.into()))?;
            let lines = if list.routines.is_empty() {
                vec!["no routines".to_owned()]
            } else {
                text::routine_table(&list.routines)
            };
            output.success(&list, &lines);
        }
        RoutineCommand::Add(args) => {
            let routine = client
                .team_routine_add(&add_params(args))
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&routine, &[text::routine_line(&routine, "added")]);
        }
        RoutineCommand::Update(args) => {
            let routine = client
                .team_routine_update(&update_params(args))
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&routine, &[text::routine_line(&routine, "updated")]);
        }
        RoutineCommand::Rm { team, routine } => {
            let removed = client
                .team_routine_remove(&RoutineRef { team, routine })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&removed, &[text::routine_line(&removed, "removed")]);
        }
        RoutineCommand::Pause { team, routine } => {
            let paused = client
                .team_routine_pause(&RoutineRef { team, routine })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&paused, &[text::routine_line(&paused, "paused")]);
        }
        RoutineCommand::Resume { team, routine } => {
            let resumed = client
                .team_routine_resume(&RoutineRef { team, routine })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&resumed, &[text::routine_line(&resumed, "resumed")]);
        }
        RoutineCommand::Runs {
            team,
            routine,
            limit,
        } => {
            let runs = client
                .team_routine_runs(&RoutineRunsParams {
                    team,
                    routine,
                    limit,
                })
                .await
                .map_err(|error| too_old(error.into()))?;
            let lines = if runs.runs.is_empty() {
                vec!["no runs".to_owned()]
            } else {
                text::run_table(&runs.runs)
            };
            output.success(&runs, &lines);
        }
        RoutineCommand::Run { team, routine, now } => {
            if !now {
                return Err(CliError::new(
                    codes::REQUEST_INVALID_PARAMS,
                    "pass --now to make a run now; a routine runs on its own schedule",
                    exit::ERROR,
                ));
            }
            let run = client
                .team_routine_run(&RoutineRef { team, routine })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&run, &[text::run_line(&run)]);
        }
    }
    Ok(())
}

/// The `team.routine_*` methods came after the first `teams/1` methods, so an older host answers them as unknown.
fn too_old(error: CliError) -> CliError {
    if error.code == codes::METHOD_NOT_FOUND {
        return CliError {
            message: "this host is too old for routines; update it".to_owned(),
            ..error
        };
    }
    error
}

fn add_params(args: RoutineAddArgs) -> RoutineAddParams {
    RoutineAddParams {
        team: args.team,
        bot: args.bot,
        name: args.name,
        prompt: args.prompt,
        cadence: args.cadence.into(),
        time: args.time,
        weekday: args.weekday.map(RoutineWeekday::from),
        month_day: args.month_day,
        run_date: args.date,
        zone: args.zone,
    }
}

fn update_params(args: RoutineUpdateArgs) -> RoutineUpdateParams {
    RoutineUpdateParams {
        team: args.team,
        routine: args.routine,
        bot: args.bot,
        name: args.name,
        prompt: args.prompt,
        cadence: args.cadence.map(RoutineCadence::from),
        time: args.time,
        weekday: args.weekday.map(RoutineWeekday::from),
        month_day: args.month_day,
        run_date: args.date,
        zone: args.zone,
    }
}
