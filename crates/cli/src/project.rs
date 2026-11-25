//! `massorca project add|list|show|inspect|rm|set-base|init-git`: registering, unregistering, inspecting, setting the base of, and initialising git in projects through the service.

use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectInspectParams, ProjectInspection, ProjectKind,
    ProjectRef, ProjectRemoveParams, ProjectSetBaseParams, ProjectUpdateParams,
};
use massorca_protocol::methods::workspaces::WorkspaceListParams;

use crate::args::{ProjectAddArgs, ProjectCommand};
use crate::connect::connect;
use crate::cwd_path::PathRules;
use crate::error::CliError;
use crate::output::Output;
use crate::project_icon;
use crate::target::Target;
use crate::workspace_text;

/// Runs one `project` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: ProjectCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let paths = PathRules::for_target(target);
    match command {
        ProjectCommand::Add(args) => {
            let params = add_params(args, paths)?;
            let project = client.project_add(&params).await?;
            if params.kind.is_none() && project.kind == ProjectKind::Folder {
                output.progress(&workspace_text::plain_folder_warning(&project.path));
            }
            let mut lines = vec![workspace_text::project_line(&project, "added")];
            if project.icon.is_some() {
                lines.push(workspace_text::project_icon_line(&project));
            }
            output.success(&project, &lines);
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
            let project = paths.selector(&project)?;
            let shown = client.project_show(&ProjectRef { project }).await?;
            output.success(&shown, &workspace_text::project_details(&shown));
        }
        ProjectCommand::Inspect { path } => {
            let params = ProjectInspectParams {
                path: paths.path(&path)?,
            };
            let inspection: ProjectInspection = client.call(projects::INSPECT, &params).await?;
            output.success(&inspection, &workspace_text::inspection_lines(&inspection));
        }
        ProjectCommand::Rm {
            project,
            forget_workspaces,
        } => {
            let project = paths.selector(&project)?;
            let forgotten = if forget_workspaces {
                let list = WorkspaceListParams {
                    project: Some(project.clone()),
                };
                Some(client.workspace_list(&list).await?.workspaces.len())
            } else {
                None
            };
            let params = ProjectRemoveParams {
                project,
                forget_workspaces,
            };
            let removed: Project = client.call(projects::REMOVE, &params).await?;
            let mut lines = vec![workspace_text::project_line(&removed, "removed")];
            if let Some(count) = forgotten {
                lines.push(workspace_text::forgotten_line(count));
            }
            output.success(&removed, &lines);
        }
        ProjectCommand::SetBase {
            project,
            base,
            clear,
        } => {
            let project = paths.selector(&project)?;
            let base = if clear { None } else { base };
            let updated: Project = client
                .call(projects::SET_BASE, &ProjectSetBaseParams { project, base })
                .await?;
            output.success(&updated, &[workspace_text::project_base_line(&updated)]);
        }
        ProjectCommand::Update {
            project,
            icon,
            clear_icon,
        } => {
            let project = paths.selector(&project)?;
            let icon = match (icon, clear_icon) {
                (Some(spec), _) => Some(Some(project_icon::parse(&spec, &paths)?)),
                (None, true) => Some(None),
                (None, false) => None,
            };
            let updated: Project = client
                .call(projects::UPDATE, &ProjectUpdateParams { project, icon })
                .await?;
            output.success(&updated, &[workspace_text::project_icon_line(&updated)]);
        }
        ProjectCommand::InitGit { project } => {
            let project = paths.selector(&project)?;
            let initialized: Project = client
                .call(projects::INIT_GIT, &ProjectRef { project })
                .await?;
            output.success(
                &initialized,
                &[workspace_text::project_line(
                    &initialized,
                    "initialized Git for",
                )],
            );
        }
    }
    Ok(())
}

/// The protocol params for `project add`, with every path made absolute against the working directory.
fn add_params(args: ProjectAddArgs, paths: PathRules) -> Result<ProjectAddParams, CliError> {
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
        .map(|path| paths.path(path))
        .transpose()?;
    let icon = args
        .icon
        .as_deref()
        .map(|spec| project_icon::parse(spec, &paths))
        .transpose()?;
    Ok(ProjectAddParams {
        icon,
        path: paths.path(&args.path)?,
        name: args.name,
        kind,
        workspaces_root,
    })
}
