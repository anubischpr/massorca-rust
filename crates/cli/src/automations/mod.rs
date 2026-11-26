//! `massorca automations ...`: scheduled automations (decision 0123). Creates, edits, shows, lists, removes, enables and
//! disables them; runs one now and lists its runs (decision 0125).

mod options;
mod text;

use clap::Subcommand;
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{
    AutomationCadence, AutomationCreateParams, AutomationRef, AutomationRunsParams,
    AutomationUpdateParams, AutomationWeekday,
};

pub use options::{CadenceArg, CreateArgs, EditArgs, WeekdayArg};

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum AutomationsCommand {
    /// Creates a scheduled automation in a project. It starts enabled, with its next run from its schedule.
    Create(CreateArgs),
    /// Changes an automation. Only the options given change; `--zone ""` returns it to the host's zone.
    Edit(EditArgs),
    /// Shows one automation, by id or exact name.
    Show {
        /// Automation id or exact name.
        automation: String,
    },
    /// Lists the automations, in creation order.
    List,
    /// Removes an automation. It cannot be restored.
    Remove {
        /// Automation id or exact name.
        automation: String,
    },
    /// Enables an automation and schedules its next run from now.
    Enable {
        /// Automation id or exact name.
        automation: String,
    },
    /// Disables an automation; it has no next run until it is enabled.
    Disable {
        /// Automation id or exact name.
        automation: String,
    },
    /// Runs an automation now. The schedule does not move; `--now` is required.
    Run {
        /// Automation id or exact name.
        automation: String,
        /// Confirms the run now.
        #[arg(long)]
        now: bool,
    },
    /// Lists an automation's runs, newest first.
    Runs {
        /// Automation id or exact name.
        automation: String,
        /// Runs to list, 1 to 100; 20 when unset.
        #[arg(long)]
        limit: Option<u32>,
        /// Lists the runs numbered below this one, the next page.
        #[arg(long)]
        before: Option<i64>,
    },
}

/// Runs one `automations` subcommand.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: AutomationsCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    match command {
        AutomationsCommand::Create(args) => {
            let created = client
                .automation_create(&create_params(args))
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&created, &[text::automation_line(&created, "created")]);
        }
        AutomationsCommand::Edit(args) => {
            check_time_given(&client, &args).await?;
            let edited = client
                .automation_update(&update_params(args))
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&edited, &[text::automation_line(&edited, "updated")]);
        }
        AutomationsCommand::Show { automation } => {
            let shown = client
                .automation_show(&AutomationRef { automation })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&shown, &text::automation_details(&shown));
        }
        AutomationsCommand::List => {
            let list = client
                .automation_list()
                .await
                .map_err(|error| too_old(error.into()))?;
            let lines = if list.automations.is_empty() {
                vec!["no automations".to_owned()]
            } else {
                text::automation_table(&list.automations)
            };
            output.success(&list, &lines);
        }
        AutomationsCommand::Remove { automation } => {
            let removed = client
                .automation_remove(&AutomationRef { automation })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&removed, &[text::automation_line(&removed, "removed")]);
        }
        AutomationsCommand::Enable { automation } => {
            let enabled = client
                .automation_enable(&AutomationRef { automation })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&enabled, &[text::automation_line(&enabled, "enabled")]);
        }
        AutomationsCommand::Disable { automation } => {
            let disabled = client
                .automation_disable(&AutomationRef { automation })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&disabled, &[text::automation_line(&disabled, "disabled")]);
        }
        AutomationsCommand::Run { automation, now } => {
            if !now {
                return Err(CliError::new(
                    codes::REQUEST_INVALID_PARAMS,
                    "pass --now to run it now; an automation runs on its own schedule",
                    exit::ERROR,
                ));
            }
            let run = client
                .automation_run_now(&AutomationRef { automation })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&run, &[text::run_line(&run)]);
        }
        AutomationsCommand::Runs {
            automation,
            limit,
            before,
        } => {
            let list = client
                .automation_runs(&AutomationRunsParams {
                    automation,
                    limit,
                    before,
                })
                .await
                .map_err(|error| too_old(error.into()))?;
            output.success(&list, &text::run_table(&list));
        }
    }
    Ok(())
}

/// The `automation.*` methods are newer than the first Bot Teams methods, and the run methods newer still, so an older
/// host answers them as unknown.
fn too_old(error: CliError) -> CliError {
    if error.code == codes::METHOD_NOT_FOUND {
        return CliError {
            message: "this host is too old for automations; update it".to_owned(),
            ..error
        };
    }
    error
}

/// A daily, hourly, weekdays or weekly schedule needs a time, and a cron automation keeps none. An edit that makes one
/// of those without `--time` is refused here, so the message can name the option to add.
async fn check_time_given(client: &Client, args: &EditArgs) -> Result<(), CliError> {
    let Some(cadence) = args.cadence else {
        return Ok(());
    };
    if args.time.is_some() || matches!(cadence, CadenceArg::Cron) {
        return Ok(());
    }
    let current = client
        .automation_show(&AutomationRef {
            automation: args.automation.clone(),
        })
        .await
        .map_err(|error| too_old(error.into()))?;
    if current.cadence == AutomationCadence::Cron {
        return Err(CliError::new(
            "usage.missing_time",
            format!("{} needs --time HH:MM", schedule_phrase(cadence)),
            exit::USAGE,
        ));
    }
    Ok(())
}

fn schedule_phrase(cadence: CadenceArg) -> &'static str {
    match cadence {
        CadenceArg::Hourly => "an hourly schedule",
        CadenceArg::Daily => "a daily schedule",
        CadenceArg::Weekdays => "a weekdays schedule",
        CadenceArg::Weekly => "a weekly schedule",
        CadenceArg::Cron => "a cron schedule",
    }
}

fn create_params(args: CreateArgs) -> AutomationCreateParams {
    AutomationCreateParams {
        project: args.project,
        name: args.name,
        prompt: args.prompt,
        agent: args.agent,
        cadence: args.cadence.into(),
        time: args.time,
        weekday: args.weekday.map(AutomationWeekday::from),
        cron: args.cron,
        zone: args.zone,
        grace_minutes: args.grace,
        creation_key: args.creation_key,
    }
}

fn update_params(args: EditArgs) -> AutomationUpdateParams {
    AutomationUpdateParams {
        automation: args.automation,
        project: args.project,
        name: args.name,
        prompt: args.prompt,
        agent: args.agent,
        cadence: args.cadence.map(AutomationCadence::from),
        time: args.time,
        weekday: args.weekday.map(AutomationWeekday::from),
        cron: args.cron,
        zone: args.zone,
        grace_minutes: args.grace,
    }
}
