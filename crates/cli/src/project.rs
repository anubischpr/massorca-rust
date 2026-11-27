//! `massorca project add|list|show|inspect|rm|set-base|update|init-git|trust-scripts`: registering, unregistering, inspecting and setting the base of projects, initialising git in folder projects, and trusting their `orca.yaml` scripts, through the service.

use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectInspectParams, ProjectInspection, ProjectKind,
    ProjectRef, ProjectRemoveParams, ProjectScript, ProjectScripts, ProjectScriptsParams,
    ProjectSetBaseParams, ProjectTrustScriptParams, ProjectUpdateParams,
};
use massorca_protocol::methods::workspaces::WorkspaceListParams;

use crate::args::{ProjectAddArgs, ProjectCommand};
use crate::connect::connect;
use crate::cwd_path::PathRules;
use crate::error::{CliError, exit};
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
        ProjectCommand::Scripts { project, workspace } => {
            let params = scripts_params(&project, workspace.as_deref(), &paths)?;
            let shown: ProjectScripts = client.call(projects::SCRIPTS, &params).await?;
            output.success(
                &shown,
                &script_lines(&shown, &project, workspace.as_deref()),
            );
        }
        ProjectCommand::TrustScripts {
            project,
            workspace,
            hashes,
        } => {
            let params = scripts_params(&project, workspace.as_deref(), &paths)?;
            let mut shown: ProjectScripts = client.call(projects::SCRIPTS, &params).await?;
            if !hashes.is_empty() {
                for hash in &hashes {
                    let script = shown_with_hash(&shown, hash)?;
                    let trust = ProjectTrustScriptParams {
                        project: params.project.clone(),
                        kind: script.kind,
                        hash: script.hash.clone(),
                        workspace: params.workspace.clone(),
                    };
                    let _: ProjectScript = client.call(projects::TRUST_SCRIPT, &trust).await?;
                }
                shown = client.call(projects::SCRIPTS, &params).await?;
            }
            output.success(
                &shown,
                &script_lines(&shown, &project, workspace.as_deref()),
            );
        }
    }
    Ok(())
}

fn scripts_params(
    project: &str,
    workspace: Option<&str>,
    paths: &PathRules,
) -> Result<ProjectScriptsParams, CliError> {
    Ok(ProjectScriptsParams {
        project: paths.selector(project)?,
        workspace: workspace.map(|text| paths.selector(text)).transpose()?,
    })
}

/// The shown script whose hash is `hash`; trusting only what was shown keeps the owner's trust on the content read.
fn shown_with_hash<'a>(
    shown: &'a ProjectScripts,
    hash: &str,
) -> Result<&'a ProjectScript, CliError> {
    let wanted = hash.trim().to_ascii_lowercase();
    shown
        .scripts
        .iter()
        .find(|script| script.hash == wanted)
        .ok_or_else(|| {
            CliError::new(
                "usage.unknown_script_hash",
                format!(
                    "no script in {} has hash {hash}; run without --hash to see the scripts and their hashes",
                    shown.source
                ),
                exit::USAGE,
            )
        })
}

/// Each script with its hash and trust, then how to trust the ones that are not.
fn script_lines(shown: &ProjectScripts, project: &str, workspace: Option<&str>) -> Vec<String> {
    if shown.scripts.is_empty() {
        return vec![format!(
            "{} defines no setup or archive script",
            shown.source
        )];
    }
    let mut lines = vec![format!("scripts in {}", shown.source)];
    let from = workspace.map_or_else(String::new, |workspace| format!(" --workspace {workspace}"));
    for script in &shown.scripts {
        let state = if script.trusted {
            "trusted"
        } else if script.changed {
            "changed since it was trusted"
        } else {
            "not trusted"
        };
        lines.push(String::new());
        lines.push(format!(
            "{} script ({state}), sha256 {}:",
            script.kind.as_str(),
            script.hash
        ));
        lines.extend(script.script.lines().map(|line| format!("    {line}")));
        if !script.trusted {
            lines.push(format!(
                "trust it: massorca project trust-scripts {project}{from} --hash {}",
                script.hash
            ));
        }
    }
    lines
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
