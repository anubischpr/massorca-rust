//! `massorca calendar`: the runs of routines and automations in a window of days, from the shared schedule (decision
//! 0124). Done routine runs are listed only with `--done`.

mod text;
mod window;

use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use massorca_protocol::codes;
use massorca_protocol::methods::calendar::CalendarKind;

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;

/// The options of `massorca calendar`.
#[derive(Debug, Args)]
pub struct CalendarArgs {
    /// The first day, as a UTC date (`YYYY-MM-DD`); today in UTC when unset.
    #[arg(long, value_name = "DATE")]
    from: Option<String>,
    /// How many days the window covers, from 1 to 62.
    #[arg(long, value_name = "N", default_value_t = 7)]
    days: i64,
    /// Lists only routines or only automations; both when unset.
    #[arg(long, value_enum)]
    kind: Option<KindArg>,
    /// Also lists the routine runs that are done; they are hidden by default.
    #[arg(long)]
    done: bool,
}

/// The `--kind` values: the names the command line uses for the two kinds.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KindArg {
    Routines,
    Automations,
}

impl From<KindArg> for CalendarKind {
    fn from(kind: KindArg) -> Self {
        match kind {
            KindArg::Routines => Self::Routine,
            KindArg::Automations => Self::Automation,
        }
    }
}

/// Runs `massorca calendar`.
pub async fn execute(
    target: &Target,
    autostart: bool,
    args: CalendarArgs,
    output: Output,
) -> Result<(), CliError> {
    let params = window::params(
        args.from.as_deref(),
        args.days,
        args.kind.map(CalendarKind::from),
        args.done,
        now_ms(),
    )?;
    let client = connect(target, autostart).await?;
    let list = client
        .calendar_occurrences(&params)
        .await
        .map_err(|error| too_old(error.into()))?;
    let mut lines = if list.occurrences.is_empty() {
        vec!["nothing scheduled".to_owned()]
    } else {
        text::occurrence_lines(&list.occurrences)
    };
    if list.truncated {
        lines.push("more runs than one call returns; narrow the window with --days".to_owned());
    }
    output.success(&list, &lines);
    Ok(())
}

/// The calendar came with `calendar/1`; an older host answers its method as unknown.
fn too_old(error: CliError) -> CliError {
    if error.code == codes::METHOD_NOT_FOUND {
        return CliError {
            message: "this host is too old for the calendar; update it".to_owned(),
            ..error
        };
    }
    error
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
