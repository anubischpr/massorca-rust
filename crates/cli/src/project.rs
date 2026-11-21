//! `massorca project add|list|show|rm`: registering and unregistering projects through the service.

use massorca_profile::Profile;
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectKind, ProjectRef};

use crate::args::{ProjectAddArgs, ProjectCommand};
use crate::connect::connect;
use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;
use crate::workspace_text;

/// Runs one `project` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: ProjectCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    match command {
        ProjectCommand::Add(args) => {
            let params = add_params(args)?;
            let project = client.project_add(&params).await?;
            output.success(&project, &[workspace_text::project_line(&project, "added")]);
        }
        ProjectCommand::List => {
            let list = client.project_list().await?;
            let lines = if list.projects.is_empty() {
                vec!["no projects".to_owned()]
            } else {
                workspace_text::project_table(&list.projects)
            };
            output.success(&list, &lines);
        }
        ProjectCommand::Show { project } => {
            let shown = client.project_show(&ProjectRef { project }).await?;
            output.success(&shown, &workspace_text::project_details(&shown));
        }
        ProjectCommand::Rm { project } => {
            let removed: Project = client.project_remove(&ProjectRef { project }).await?;
            output.success(
                &removed,
                &[workspace_text::project_line(&removed, "removed")],
            );
        }
    }
    Ok(())
}

/// The protocol params for `project add`, with every path made absolute against the working directory.
fn add_params(args: ProjectAddArgs) -> Result<ProjectAddParams, CliError> {
    let kind = if args.folder {
        Some(ProjectKind::Folder)
    } else if args.git {
        Some(ProjectKind::Git)
    } else {
        None
    };
    let workspaces_root = args
        .workspaces_root
        .as_deref()
        .map(cwd_path::absolute)
        .transpose()?;
    Ok(ProjectAddParams {
        path: cwd_path::absolute(&args.path)?,
        name: args.name,
        kind,
        workspaces_root,
    })
}
