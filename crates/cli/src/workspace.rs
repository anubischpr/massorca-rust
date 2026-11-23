//! `massorca workspace create|list|show|rm` (alias `ws`): creating and removing workspaces through the service.

use massorca_protocol::methods::workspaces::{
    Workspace, WorkspaceCreateParams, WorkspaceListParams, WorkspaceRef, WorkspaceRemoveParams,
};

use crate::args::{WorkspaceCommand, WorkspaceCreateArgs};
use crate::connect::connect;
use crate::cwd_path::PathRules;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::workspace_text;

/// Runs one `workspace` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: WorkspaceCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let paths = PathRules::for_target(target);
    match command {
        WorkspaceCommand::Create(args) => {
            let params = create_params(args, paths)?;
            let created = client.workspace_create(&params).await?;
            output.success(
                &created,
                &[workspace_text::workspace_line(&created, "created")],
            );
        }
        WorkspaceCommand::List(args) => {
            let list = client
                .workspace_list(&WorkspaceListParams {
                    project: args
                        .project
                        .as_deref()
                        .map(|text| paths.selector(text))
                        .transpose()?,
                })
                .await?;
            let mut lines = if list.workspaces.is_empty() {
                vec!["no workspaces".to_owned()]
            } else {
                workspace_text::workspace_table(&list.workspaces)
            };
            lines.extend(
                list.unregistered
                    .iter()
                    .map(workspace_text::unregistered_line),
            );
            output.success(&list, &lines);
        }
        WorkspaceCommand::Show { workspace } => {
            let workspace = paths.selector(&workspace)?;
            let shown: Workspace = client.workspace_show(&WorkspaceRef { workspace }).await?;
            output.success(&shown, &workspace_text::workspace_details(&shown));
        }
        WorkspaceCommand::Rm {
            workspace,
            force,
            discard_ignored,
            forget_orphan,
        } => {
            let removed = client
                .workspace_remove(&WorkspaceRemoveParams {
                    workspace: paths.selector(&workspace)?,
                    force,
                    discard_ignored,
                    forget_orphan,
                })
                .await?;
            output.success(&removed, &workspace_text::removal_lines(&removed));
        }
    }
    Ok(())
}

/// The protocol params for `workspace create`, with a worktree path made absolute against the working directory.
fn create_params(
    args: WorkspaceCreateArgs,
    paths: PathRules,
) -> Result<WorkspaceCreateParams, CliError> {
    let path = args
        .path
        .as_deref()
        .map(|path| paths.path(path))
        .transpose()?;
    Ok(WorkspaceCreateParams {
        project: paths.selector(&args.project)?,
        name: args.name.unwrap_or_default(),
        branch: args.branch,
        base: args.base,
        existing_branch: args.existing_branch,
        path,
        request_id: args.request_id,
    })
}
