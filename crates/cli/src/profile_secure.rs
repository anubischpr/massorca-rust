//! `massorca profile secure`: the owner's explicit way to make an existing home private (ADR 0073). Without `--yes` it
//! prints what it would change and changes nothing; with `--yes` it applies that and checks the profile as a start does.

use massorca_profile::{FolderPlan, Profile, RootAccess, apply_secure, plan_secure};
use serde_json::{Value, json};

use crate::args::ProfileSecureArgs;
use crate::error::CliError;
use crate::output::Output;

pub fn run(profile: &Profile, args: &ProfileSecureArgs, output: Output) -> Result<(), CliError> {
    let plan = plan_secure(profile)?;
    let mut lines = plan_lines(&plan);
    if !args.yes {
        lines.push(
            "Nothing was changed. Run `massorca profile secure --yes` to apply this.".to_owned(),
        );
        output.success(
            &json!({ "applied": false, "folders": folders(&plan) }),
            &lines,
        );
        return Ok(());
    }
    let access = apply_secure(profile)?;
    lines.push(
        "The profile is private now. Its files are kept as they are and trusted from now on."
            .to_owned(),
    );
    output.success(
        &json!({ "applied": true, "folders": folders(&plan), "root": root_access(access) }),
        &lines,
    );
    Ok(())
}

fn plan_lines(plan: &[FolderPlan]) -> Vec<String> {
    let mut lines = Vec::new();
    for folder in plan {
        lines.push(format!("{}:", folder.path.display()));
        if !folder.changes() {
            lines.push("  already private".to_owned());
            continue;
        }
        lines.extend(
            folder
                .remove
                .iter()
                .map(|entry| format!("  remove {entry}")),
        );
        lines.extend(folder.keep.iter().map(|entry| format!("  keep   {entry}")));
    }
    lines
}

fn folders(plan: &[FolderPlan]) -> Value {
    plan.iter()
        .map(|folder| {
            json!({
                "path": folder.path.display().to_string(),
                "remove": folder.remove,
                "keep": folder.keep,
            })
        })
        .collect()
}

fn root_access(access: RootAccess) -> &'static str {
    match access {
        RootAccess::Created => "created",
        RootAccess::Private => "private",
        RootAccess::Repaired => "repaired",
    }
}
