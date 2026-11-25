//! The install review of a catalogue package: the lines that say exactly what the service reported (decision 0062).
//! Permissions, the agent programs, the vendor commands and the fingerprint are shown as the service sent them.

use massorca_protocol::methods::extensions::{
    CliPlan, CliPlanState, ExtensionReview, KindView, RiskLevel,
};

use super::extension_items::{permission_word, risk_word, source_word};

/// The review, one line per fact, in the order the person reads it: what the package is, its risk and permissions,
/// what each agent runs, each vendor command, and the consent the person accepts.
pub fn review_lines(review: &ExtensionReview) -> Vec<String> {
    let (level, reasons) = shown_risk(review);
    let mut lines: Vec<String> = outcome_line(review).into_iter().collect();
    lines.extend([
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
    ]);
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
    // A CLI that is already here has no lines of its own: the outcome line says it is found and not installed.
    lines.extend(
        review
            .cli
            .iter()
            .filter(|plan| !matches!(plan.state, CliPlanState::Detected { .. }))
            .flat_map(cli_lines),
    );
    lines
}

/// The fingerprints of the review, shown under Details with what each one is.
pub fn review_details(review: &ExtensionReview) -> Vec<String> {
    vec![
        format!("Package SHA-256: {}", review.sha256),
        format!(
            "Consent fingerprint (this review, as accepted): {}",
            review.consent
        ),
    ]
}

/// Whether this computer runs a vendor installer: a CLI plan that will run its command.
pub fn installer_runs(review: &ExtensionReview) -> bool {
    review
        .cli
        .iter()
        .any(|plan| matches!(plan.state, CliPlanState::WillRun { .. }))
}

/// The button that finishes the review: Allow and install when an installer runs, else Add to MassOrca.
pub fn install_button_label(review: &ExtensionReview) -> &'static str {
    if installer_runs(review) {
        "Allow and install"
    } else {
        "Add to MassOrca"
    }
}

/// The first line: what installing does on this computer, in plain words.
fn outcome_line(review: &ExtensionReview) -> Option<String> {
    let commands: Vec<String> = review
        .cli
        .iter()
        .filter_map(|plan| match &plan.state {
            CliPlanState::WillRun { command, .. } => Some(command.join(" ")),
            _ => None,
        })
        .collect();
    if !commands.is_empty() {
        return Some(format!(
            "MassOrca will run the vendor's installer: {}.",
            commands.join("; ")
        ));
    }
    review
        .cli
        .iter()
        .any(|plan| matches!(plan.state, CliPlanState::Detected { .. }))
        .then(|| {
            format!(
                "{} is already on this computer. MassOrca will only add it to the model picker; nothing is downloaded or installed.",
                review.name
            )
        })
}

/// How high a risk level is, to pick the highest of a review's reasons.
fn rank(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::High => 2,
        RiskLevel::Medium => 1,
        _ => 0,
    }
}

/// The risk this computer shows: the highest level of the reasons that apply here, and their copy with repeats
/// dropped. An installer reason applies only when the agent's CLI plan runs its command.
pub fn shown_risk(review: &ExtensionReview) -> (RiskLevel, Vec<String>) {
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
    let mut copies: Vec<String> = Vec::new();
    for reason in review
        .risk
        .reasons
        .iter()
        .filter(|reason| applies(&reason.subject))
    {
        if rank(reason.level) > rank(level) {
            level = reason.level;
        }
        if !copies.contains(&reason.copy) {
            copies.push(reason.copy.clone());
        }
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

/// What the install does about one agent CLI on this computer.
fn cli_lines(plan: &CliPlan) -> Vec<String> {
    let mut lines = vec![match &plan.state {
        CliPlanState::Detected { .. } => format!(
            "{}: its CLI is already on this computer; nothing is installed for it.",
            plan.agent
        ),
        CliPlanState::WillRun {
            command,
            runs_package_scripts,
        } => format!(
            "{}: this vendor command runs: {}. It installs the vendor's current release; the version is not pinned.{}",
            plan.agent,
            command.join(" "),
            if *runs_package_scripts {
                " npm also runs the install scripts of the package and its dependencies, which this review cannot show."
            } else {
                ""
            }
        ),
        CliPlanState::Unsupported => format!(
            "{}: no install command for this system; the agent is registered without its CLI.",
            plan.agent
        ),
        CliPlanState::Unknown => {
            format!("{}: this version does not know the CLI state.", plan.agent)
        }
    }];
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
    use massorca_protocol::methods::extensions::{
        CliPlanState, KindView, Listing, Permission, Risk, RiskLevel, RiskReason, SourceKind,
        SourceRef,
    };

    use super::*;

    fn review() -> ExtensionReview {
        ExtensionReview {
            id: "massorca.claude".to_owned(),
            name: "Claude".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "massorca".to_owned(),
            description: "Starts Claude sessions.".to_owned(),
            source: SourceRef {
                kind: SourceKind::Catalogue,
                reference: "massorca.claude".to_owned(),
            },
            sha256: "abc123".to_owned(),
            permissions: vec![Permission {
                kind: "files:read".to_owned(),
                reason: "Reads the project.".to_owned(),
                scope: None,
            }],
            risk: Risk {
                level: RiskLevel::High,
                reasons: Vec::new(),
            },
            listing: Listing {
                logo: None,
                category: "agents".to_owned(),
                developer: None,
                mention: None,
                banner: None,
                connections: Vec::new(),
                website: None,
                privacy: None,
                terms: None,
            },
            kinds: vec![KindView {
                id: "claude".to_owned(),
                readable: true,
                program: vec!["claude".to_owned()],
                args: vec!["--json".to_owned()],
                env: vec!["CLAUDE_HOME".to_owned()],
                credential_home_env: None,
                own_credential_home: true,
                credential_files: Vec::new(),
            }],
            mcp_servers: Vec::new(),
            cli: vec![CliPlan {
                agent: "claude".to_owned(),
                binary: Some("claude".to_owned()),
                verify: Some(vec!["claude".to_owned(), "--version".to_owned()]),
                uninstall: None,
                state: CliPlanState::WillRun {
                    command: vec![
                        "winget".to_owned(),
                        "install".to_owned(),
                        "Anthropic.ClaudeCode".to_owned(),
                    ],
                    runs_package_scripts: false,
                },
            }],
            runs_installer: true,
            consent: "fingerprint-1".to_owned(),
        }
    }

    #[test]
    fn a_command_that_runs_package_scripts_says_so_and_one_that_does_not_stays_quiet() {
        let mut plan = review().cli.remove(0);
        let quiet = cli_lines(&plan);
        assert!(!quiet[0].contains("install scripts"), "{quiet:?}");
        plan.state = CliPlanState::WillRun {
            command: vec![
                "npm".to_owned(),
                "install".to_owned(),
                "-g".to_owned(),
                "x".to_owned(),
            ],
            runs_package_scripts: true,
        };
        let told = cli_lines(&plan);
        assert!(
            told[0].ends_with(
                "npm also runs the install scripts of the package and its dependencies, which this review cannot show."
            ),
            "{told:?}"
        );
    }

    #[test]
    fn the_review_shows_the_permissions_programs_commands_and_consent_it_was_given() {
        let lines = review_lines(&review());
        assert!(lines.contains(&"Permissions:".to_owned()), "{lines:?}");
        assert!(
            lines.contains(&"Files (read): Reads the project.".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"Agent claude starts: claude --json".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(
                &"Agent claude reads these environment variables: CLAUDE_HOME".to_owned()
            ),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"claude: this vendor command runs: winget install Anthropic.ClaudeCode. It installs the vendor's current release; the version is not pinned.".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"Verify after the install: claude --version".to_owned()),
            "{lines:?}"
        );
    }

    /// The review of a CLI already on this computer: the installer is skipped.
    fn detected_review() -> ExtensionReview {
        let mut found = review();
        found.cli[0].state = CliPlanState::Detected { path: None };
        found.runs_installer = false;
        found
    }

    /// The review with the risk reasons the service gives an agent that starts a program and has an installer.
    fn with_installer_risk(mut risky: ExtensionReview) -> ExtensionReview {
        let program = "Runs a program on this computer with your rights.";
        risky.risk = Risk {
            level: RiskLevel::High,
            reasons: vec![
                RiskReason {
                    level: RiskLevel::High,
                    subject: "permission:process".to_owned(),
                    copy: program.to_owned(),
                },
                RiskReason {
                    level: RiskLevel::High,
                    subject: "agentAdapters:claude".to_owned(),
                    copy: program.to_owned(),
                },
                RiskReason {
                    level: RiskLevel::High,
                    subject: "agentAdapters:claude:install".to_owned(),
                    copy: "Runs the vendor's installer on this computer.".to_owned(),
                },
            ],
        };
        risky
    }

    #[test]
    fn a_cli_already_here_is_said_plainly_and_nothing_is_installed() {
        let lines = review_lines(&detected_review());
        assert_eq!(
            lines[0],
            "Claude is already on this computer. MassOrca will only add it to the model picker; nothing is downloaded or installed.",
            "{lines:?}"
        );
    }

    #[test]
    fn a_missing_cli_names_the_installer_that_will_run() {
        let lines = review_lines(&review());
        assert_eq!(
            lines[0],
            "MassOrca will run the vendor's installer: winget install Anthropic.ClaudeCode.",
            "{lines:?}"
        );
    }

    #[test]
    fn the_button_adds_to_massorca_unless_an_installer_runs() {
        assert_eq!(install_button_label(&detected_review()), "Add to MassOrca");
        assert_eq!(install_button_label(&review()), "Allow and install");
    }

    #[test]
    fn a_skipped_installer_adds_no_installer_risk() {
        let (_, copies) = shown_risk(&with_installer_risk(detected_review()));
        assert!(
            !copies
                .iter()
                .any(|copy| copy == "Runs the vendor's installer on this computer."),
            "{copies:?}"
        );
    }

    #[test]
    fn an_installer_that_runs_keeps_its_risk() {
        let (level, copies) = shown_risk(&with_installer_risk(review()));
        assert!(
            copies
                .iter()
                .any(|copy| copy == "Runs the vendor's installer on this computer."),
            "{copies:?}"
        );
        assert_eq!(level, RiskLevel::High);
    }

    #[test]
    fn a_program_line_is_listed_once() {
        let (_, copies) = shown_risk(&with_installer_risk(detected_review()));
        let program = "Runs a program on this computer with your rights.";
        assert_eq!(
            copies
                .iter()
                .filter(|copy| copy.as_str() == program)
                .count(),
            1,
            "{copies:?}"
        );
    }

    #[test]
    fn the_hashes_sit_under_details_with_their_labels() {
        let summary = review_lines(&review());
        assert!(
            !summary
                .iter()
                .any(|line| line.contains("SHA-256") || line.starts_with("Consent")),
            "{summary:?}"
        );
        assert_eq!(
            review_details(&review()),
            vec![
                "Package SHA-256: abc123".to_owned(),
                "Consent fingerprint (this review, as accepted): fingerprint-1".to_owned(),
            ]
        );
    }

    #[test]
    fn a_package_that_asks_for_nothing_says_so() {
        let mut plain = review();
        plain.permissions.clear();
        plain.cli.clear();
        plain.kinds.clear();
        let lines = review_lines(&plain);
        assert!(lines.contains(&"Permissions: none".to_owned()), "{lines:?}");
    }
}
