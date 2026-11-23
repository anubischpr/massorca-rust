//! The install review of a catalogue package: the lines that say exactly what the service reported (decision 0062).
//! Permissions, the agent programs, the vendor commands and the fingerprint are shown as the service sent them.

use massorca_protocol::methods::extensions::{CliPlan, CliPlanState, ExtensionReview, KindView};

use super::extension_items::{permission_word, risk_word, source_word};

/// The review, one line per fact, in the order the person reads it: what the package is, its risk and permissions,
/// what each agent runs, each vendor command, and the consent the person accepts.
pub fn review_lines(review: &ExtensionReview) -> Vec<String> {
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
        format!("SHA-256: {}", review.sha256),
        format!("Risk: {}", risk_word(review.risk.level)),
    ];
    lines.extend(review.risk.reasons.iter().map(|reason| reason.copy.clone()));
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
    lines.push(format!("Consent: {}", review.consent));
    lines
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
        CliPlanState, KindView, Listing, Permission, Risk, RiskLevel, SourceKind, SourceRef,
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
        assert!(lines.contains(&"SHA-256: abc123".to_owned()), "{lines:?}");
        assert_eq!(
            lines.last().map(String::as_str),
            Some("Consent: fingerprint-1")
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
