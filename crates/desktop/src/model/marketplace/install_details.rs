//! What the install review's Show details holds (decision 0062, owner ruling 2026-11-24): every technical fact the
//! service reported, one line each, with repeats dropped. The dialog shows them only when the person opens them.

use massorca_protocol::methods::extensions::{
    CliPlan, CliPlanState, ExtensionReview, KindView, RiskLevel,
};

use super::extension_items::{permission_word, risk_word, source_word};

/// The details, in the order the person reads them: the package, its source and risk, the capability lines, the
/// permissions, what each agent runs, each vendor command, and the fingerprints with their labels.
#[must_use]
pub fn review_details(review: &ExtensionReview) -> Vec<String> {
    let (level, reasons) = shown_risk(review);
    let mut lines = vec![
        format!(
            "{} ({}) {} by {}",
            review.name, review.id, review.version, review.publisher
        ),
        review.description.clone(),
        format!(
            "Source: {} {}",
            source_word(review.source.kind),
            review.source.reference
        ),
        format!("Risk: {}", risk_word(level)),
    ];
    lines.extend(reasons);
    if review.permissions.is_empty() {
        lines.push("Permissions: none".to_owned());
    } else {
        lines.push("Permissions:".to_owned());
        lines.extend(review.permissions.iter().map(|permission| {
            format!(
                "{}: {}",
                permission_word(&permission.kind),
                permission.reason
            )
        }));
    }
    lines.extend(review.kinds.iter().flat_map(kind_lines));
    lines.extend(review.mcp_servers.iter().map(|server| {
        format!(
            "MCP server {} runs: {}",
            server.id,
            server.command.join(" ")
        )
    }));
    lines.extend(review.cli.iter().flat_map(cli_lines));
    lines.push(format!("Package SHA-256: {}", review.sha256));
    lines.push(format!(
        "Consent fingerprint (this review, as accepted): {}",
        review.consent
    ));
    let mut seen = Vec::with_capacity(lines.len());
    for line in lines {
        if !line.is_empty() && !seen.contains(&line) {
            seen.push(line);
        }
    }
    seen
}

fn rank(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::High => 2,
        RiskLevel::Medium => 1,
        _ => 0,
    }
}

/// The highest level of the reasons that apply here, and their copy. An installer reason applies only when the
/// agent's CLI plan runs its command.
fn shown_risk(review: &ExtensionReview) -> (RiskLevel, Vec<String>) {
    let applies = |subject: &str| {
        let installer = subject
            .strip_prefix("agentAdapters:")
            .and_then(|rest| rest.strip_suffix(":install"));
        installer.is_none_or(|agent| {
            review.cli.iter().any(|plan| {
                plan.agent == agent && matches!(plan.state, CliPlanState::WillRun { .. })
            })
        })
    };
    let mut level = RiskLevel::Low;
    let mut copies = Vec::new();
    for reason in review
        .risk
        .reasons
        .iter()
        .filter(|reason| applies(&reason.subject))
    {
        if rank(reason.level) > rank(level) {
            level = reason.level;
        }
        copies.push(reason.copy.clone());
    }
    (level, copies)
}

/// An agent kind's program and the names of what it reads; values are never shown.
fn kind_lines(kind: &KindView) -> Vec<String> {
    let mut lines = vec![format!(
        "Agent {} starts: {}",
        kind.id,
        [kind.program.as_slice(), kind.args.as_slice()]
            .concat()
            .join(" ")
    )];
    if !kind.env.is_empty() {
        lines.push(format!(
            "Agent {} reads these environment variables: {}",
            kind.id,
            kind.env.join(", ")
        ));
    }
    if !kind.credential_files.is_empty() {
        lines.push(format!(
            "Agent {} keeps its sign-in in: {}",
            kind.id,
            kind.credential_files.join(", ")
        ));
    }
    lines
}

/// What the install does about one agent CLI. The installer line appears only when the installer will run.
fn cli_lines(plan: &CliPlan) -> Vec<String> {
    let first = match &plan.state {
        CliPlanState::Detected { .. } => None,
        CliPlanState::WillRun {
            command,
            runs_package_scripts,
        } => Some(format!(
            "{}: this vendor command runs: {}. It installs the vendor's current release; the version is not pinned.{}",
            plan.agent,
            command.join(" "),
            if *runs_package_scripts {
                " npm also runs the install scripts of the package and its dependencies, which this review cannot show."
            } else {
                ""
            }
        )),
        CliPlanState::Unsupported => Some(format!(
            "{}: no install command for this system; the agent is registered without its CLI.",
            plan.agent
        )),
        CliPlanState::Unknown => Some(format!(
            "{}: this version does not know the CLI state.",
            plan.agent
        )),
    };
    let mut lines: Vec<String> = first.into_iter().collect();
    if let Some(verify) = &plan.verify {
        lines.push(format!("Verify after the install: {}", verify.join(" ")));
    }
    if let Some(uninstall) = &plan.uninstall {
        lines.push(format!("Uninstall command: {}", uninstall.join(" ")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::extensions::{CliPlanState, Risk, RiskLevel, RiskReason};

    use super::*;
    use crate::model::marketplace::install_review::tests::{detected_review, review};

    const PROGRAM: &str = "Runs a program on this computer with your rights.";
    const INSTALLER: &str = "Runs the vendor's installer on this computer.";
    const SIGN_INS: &str = "Gives agents access to stored sign-ins.";

    fn with_reasons(mut risky: ExtensionReview) -> ExtensionReview {
        let reason = |subject: &str, copy: &str| RiskReason {
            subject: subject.to_owned(),
            copy: copy.to_owned(),
            ..crate::service::fixtures::base_risk_reason()
        };
        risky.risk = Risk {
            level: RiskLevel::High,
            reasons: vec![
                reason("permission:process", PROGRAM),
                reason("agentAdapters:claude", PROGRAM),
                reason("permission:credentials", SIGN_INS),
                reason("agentAdapters:claude:install", INSTALLER),
            ],
        };
        risky.kinds[0].credential_files = vec![".claude/.credentials.json".to_owned()];
        risky.cli[0].uninstall = Some(vec![
            "winget".to_owned(),
            "uninstall".to_owned(),
            "Anthropic.ClaudeCode".to_owned(),
        ]);
        risky
    }

    #[test]
    fn the_details_hold_every_technical_fact_once() {
        let details = review_details(&with_reasons(review()));
        for line in [
            "Claude (massorca.claude) 1.0.0 by massorca",
            "Starts Claude sessions.",
            "Source: Catalogue massorca.claude",
            "Risk: High",
            PROGRAM,
            SIGN_INS,
            INSTALLER,
            "Permissions:",
            "Files (read): Reads the project.",
            "Agent claude starts: claude --json",
            "Agent claude keeps its sign-in in: .claude/.credentials.json",
            "claude: this vendor command runs: winget install Anthropic.ClaudeCode. It installs the vendor's current release; the version is not pinned.",
            "Verify after the install: claude --version",
            "Uninstall command: winget uninstall Anthropic.ClaudeCode",
            "Package SHA-256: abc123",
            "Consent fingerprint (this review, as accepted): fingerprint-1",
        ] {
            assert!(
                details.iter().any(|shown| shown == line),
                "{line:?} missing: {details:#?}"
            );
        }
        assert_eq!(
            details
                .iter()
                .filter(|line| line.as_str() == PROGRAM)
                .count(),
            1,
            "{details:#?}"
        );
    }

    #[test]
    fn a_skipped_installer_has_no_installer_lines() {
        let details = review_details(&with_reasons(detected_review()));
        assert!(
            !details.iter().any(|line| line == INSTALLER),
            "{details:#?}"
        );
        assert!(
            !details
                .iter()
                .any(|line| line.contains("vendor command runs")),
            "{details:#?}"
        );
        assert!(
            details.iter().any(|line| line == "Risk: High"),
            "{details:#?}"
        );
        assert!(
            details
                .iter()
                .any(|line| line == "Verify after the install: claude --version"),
            "{details:#?}"
        );
    }

    #[test]
    fn a_package_that_asks_for_nothing_says_so() {
        let mut plain = review();
        plain.permissions.clear();
        plain.cli.clear();
        plain.kinds.clear();
        plain.risk.level = RiskLevel::Low;
        assert!(review_details(&plain).contains(&"Permissions: none".to_owned()));
        let _ = CliPlanState::Unknown;
    }
}
