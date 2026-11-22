//! Human-readable lines for extension items, install reviews and removals.

use massorca_protocol::methods::extensions::{
    CliPlan, CliPlanState, CliRecord, CliRemoval, CliRemovalAction, ExtensionInstalled,
    ExtensionItem, ExtensionRemoved, ExtensionReview, ExtensionState, ItemProblem, Permission,
    Risk, RiskLevel, SourceKind,
};

use crate::workspace_text;

/// Items as a table; `none` is the line for an empty list.
pub fn item_rows(items: &[ExtensionItem], none: &str) -> Vec<String> {
    if items.is_empty() {
        return vec![none.to_owned()];
    }
    let rows = items
        .iter()
        .map(|item| {
            vec![
                item.id.clone(),
                item.version.clone(),
                state_text(item.state).to_owned(),
                risk_text(item.risk.level).to_owned(),
                item.name.clone(),
            ]
        })
        .collect();
    let mut lines = workspace_text::table(&["ID", "VERSION", "STATE", "RISK", "NAME"], rows);
    lines.extend(items.iter().filter_map(|item| {
        item.problem
            .as_ref()
            .map(|problem| format!("{}: {}", item.id, problem_text(problem)))
    }));
    lines
}

/// One item in detail.
pub fn item_lines(item: &ExtensionItem) -> Vec<String> {
    let mut lines = vec![
        format!("{} ({}) {}", item.name, item.id, item.version),
        format!("state: {}", state_text(item.state)),
    ];
    lines.extend(item.problem.as_ref().map(problem_text));
    lines.extend([
        format!("publisher: {}", item.publisher),
        item.description.clone(),
    ]);
    if let Some(developer) = &item.listing.developer {
        lines.push(format!("developer: {}", developer.name));
    }
    lines.push(format!("category: {}", item.listing.category));
    lines.extend(risk_lines(&item.risk));
    for permission in &item.permissions {
        lines.push(permission_line(permission));
    }
    for agent in &item.agents {
        let found = match &agent.program_path {
            Some(path) if agent.detected => format!("detected at {path}"),
            _ => "not detected".to_owned(),
        };
        let by_package = if agent.cli_installed_by_package {
            ", installed by this package"
        } else {
            ""
        };
        lines.push(format!("agent {}: {found}{by_package}", agent.id));
        if let Some(cli) = &agent.cli {
            lines.push(format!("  vendor install docs: {}", cli.source));
        }
    }
    if let Some(installed) = &item.installed {
        lines.push(format!(
            "installed from {}:{} (sha256 {})",
            source_kind(installed.source.kind),
            installed.source.reference,
            installed.sha256
        ));
        lines.extend(installed.cli.iter().map(record_line));
    }
    let uninstall = &item.uninstall;
    if uninstall.package_files {
        lines.push("removing deletes the package files".to_owned());
    }
    if uninstall.storage {
        lines.push("removing deletes the package's data".to_owned());
    }
    for binary in &uninstall.cli_binaries {
        lines.push(format!(
            "removing can uninstall the {binary} CLI it installed"
        ));
    }
    for agent in &uninstall.offered_agent_homes {
        lines.push(format!("removing can delete the {agent} credential home"));
    }
    lines
}

/// The install review: what the package is, its risk and permissions, and what runs on this host.
pub fn review_lines(review: &ExtensionReview) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{} ({}) {} by {}",
            review.name, review.id, review.version, review.publisher
        ),
        review.description.clone(),
        format!(
            "source: {}:{}  sha256: {}",
            source_kind(review.source.kind),
            review.source.reference,
            review.sha256
        ),
    ];
    lines.extend(risk_lines(&review.risk));
    if review.permissions.is_empty() {
        lines.push("permissions: none".to_owned());
    } else {
        lines.push("permissions:".to_owned());
        lines.extend(
            review
                .permissions
                .iter()
                .map(|permission| format!("  {}", permission_line(permission))),
        );
    }
    lines.extend(review.cli.iter().map(plan_line));
    lines.push(format!("consent: {}", review.consent));
    lines
}

/// What an install did.
pub fn installed_lines(installed: &ExtensionInstalled) -> Vec<String> {
    let mut lines = vec![format!("installed {} {}", installed.id, installed.version)];
    lines.extend(installed.cli.iter().map(record_line));
    lines
}

/// What a removal did, including anything it could not undo.
pub fn removed_lines(removed: &ExtensionRemoved) -> Vec<String> {
    let mut lines = vec![format!("removed {} {}", removed.id, removed.version)];
    for failure in &removed.failures {
        lines.push(format!(
            "not undone: {}: {} ({})",
            failure.owner, failure.message, failure.code
        ));
    }
    lines.extend(removed.cli.iter().map(removal_line));
    lines
}

pub fn state_text(state: ExtensionState) -> &'static str {
    match state {
        ExtensionState::Available => "available",
        ExtensionState::Enabled => "enabled",
        ExtensionState::Disabled => "disabled",
        ExtensionState::Unknown => "unknown",
    }
}

/// An installed package whose files cannot be trusted: the message and its stable code.
fn problem_text(problem: &ItemProblem) -> String {
    format!("problem: {} ({})", problem.message, problem.code)
}

fn risk_text(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Unknown => "unknown",
    }
}

fn source_kind(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Catalogue => "catalogue",
        SourceKind::Dir => "dir",
        SourceKind::File => "file",
        SourceKind::Unknown => "unknown",
    }
}

fn risk_lines(risk: &Risk) -> Vec<String> {
    let mut lines = vec![format!("risk: {}", risk_text(risk.level))];
    lines.extend(risk.reasons.iter().map(|reason| {
        format!(
            "  {}: {}: {}",
            risk_text(reason.level),
            reason.subject,
            reason.copy
        )
    }));
    lines
}

fn permission_line(permission: &Permission) -> String {
    let scope = permission
        .scope
        .as_ref()
        .map_or_else(String::new, |scope| format!(" [{}]", scope.join(", ")));
    format!("{}{scope}: {}", permission.kind, permission.reason)
}

fn plan_line(plan: &CliPlan) -> String {
    let binary = plan.binary.as_deref().unwrap_or("-");
    match &plan.state {
        CliPlanState::Detected { path } => format!(
            "cli {} ({binary}): found at {path}, nothing runs",
            plan.agent
        ),
        CliPlanState::WillRun { command } => format!(
            "cli {} ({binary}): not found; installing runs this vendor command: {}",
            plan.agent,
            command_text(command)
        ),
        CliPlanState::Unsupported => format!(
            "cli {} ({binary}): not found, and no vendor command for this system; the agent registers without it",
            plan.agent
        ),
        CliPlanState::Unknown => format!("cli {}: a plan this client does not know", plan.agent),
    }
}

fn record_line(record: &CliRecord) -> String {
    match &record.installer {
        Some(run) => format!(
            "cli {}: installed by the package with `{}`, exit {}",
            record.agent,
            command_text(&run.command),
            run.exit_code
                .map_or_else(|| "none".to_owned(), |code| code.to_string())
        ),
        None if record.detected_before_install => {
            format!("cli {}: found on this host, nothing ran", record.agent)
        }
        None => format!("cli {}: missing", record.agent),
    }
}

fn removal_line(removal: &CliRemoval) -> String {
    let cli = format!("cli {} ({})", removal.agent, removal.binary);
    match &removal.action {
        CliRemovalAction::LeftAlone => {
            format!("{cli}: left alone, the package did not install it")
        }
        CliRemovalAction::NoCommand => {
            format!("{cli}: installed by the package, but it declares no uninstall command here")
        }
        CliRemovalAction::Offered { command } => format!(
            "{cli}: installed by the package; run `{}` to uninstall it, or remove with --uninstall-cli",
            command_text(command)
        ),
        CliRemovalAction::Ran { command, .. } => {
            format!("{cli}: uninstalled with `{}`", command_text(command))
        }
        CliRemovalAction::Failed { command, message } => format!(
            "{cli}: uninstall `{}` failed: {message}",
            command_text(command)
        ),
        CliRemovalAction::Unknown => format!("{cli}: an action this client does not know"),
    }
}

/// An argv for display. A part with anything but plain characters is quoted with Rust escaping, so quotes, spaces,
/// control characters and look-alike Unicode cannot blur where one argument ends. It is never run from this text.
pub fn command_text(command: &[String]) -> String {
    command
        .iter()
        .map(|part| {
            let plain = !part.is_empty()
                && part.chars().all(|c| {
                    c.is_ascii_alphanumeric()
                        || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | '@' | '+' | ',')
                });
            if plain {
                part.clone()
            } else {
                format!("{part:?}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
