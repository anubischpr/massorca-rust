//! `massorca automations ...`: scheduled automation definitions (decision 0123). Creates, edits, shows, lists, removes,
//! enables and disables them. A definition only: no run is started yet.

mod options;
mod text;

use clap::Subcommand;
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{
    AutomationCadence, AutomationCreateParams, AutomationRef, AutomationUpdateParams,
    AutomationWeekday,
};

pub use options::{CadenceArg, CreateArgs, EditArgs, WeekdayArg};

use crate::connect::connect;
use crate::error::CliError;
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
    }
    Ok(())
}

/// The `automation.*` methods are newer than the first Bot Teams methods, so an older host answers them as unknown.
fn too_old(error: CliError) -> CliError {
    if error.code == codes::METHOD_NOT_FOUND {
        return CliError {
            message: "this host is too old for automations; update it".to_owned(),
            ..error
        };
    }
    error
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
