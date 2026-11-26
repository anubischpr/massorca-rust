//! Agent packages (`agentAdapters`) and MCP servers (`mcpServers`): the process tier of §3.
//!
//! The package declares what to run. It never parses the agent declaration's fields (core-agents does), and it never
//! bundles a vendor binary: a `cli` block names how the CLI is obtained, and the install runs only after the user
//! confirms.

use std::collections::HashSet;

use serde::Deserialize;

use super::contributes::list;
use super::{Manifest, is_https_url};
use crate::display_chars;
use crate::error::{Problems, at};
use crate::ids;
use crate::permission::PermissionKind;

/// Agent ids reserved to the publisher `massorca`, which ships the built-in agents of 0050.
pub const RESERVED_AGENT_IDS: [&str; 4] = ["claude", "codex", "antigravity", "gentle-shell"];

/// The publisher allowed to use the reserved agent ids.
pub const RESERVED_PUBLISHER: &str = "massorca";

/// The programs a vendor install or uninstall command may start, on every OS (0062, second review). A command names one
/// of these bare, never a path; the manifest refuses any other, and the install refuses it again before it runs.
pub const VENDOR_PROGRAMS: [&str; 5] = ["winget", "brew", "npm", "powershell", "bash"];

/// Whether a cli block declares a vendor command: an install, an uninstall or a verify. Only a built-in may (0062, V1).
pub fn declares_vendor_commands(cli: &AgentCli) -> bool {
    cli.verify.is_some() || !cli.install.is_empty() || !cli.uninstall.is_empty()
}

/// Whether a vendor command's argv starts with an allowlisted program.
pub fn is_vendor_program(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|program| VENDOR_PROGRAMS.contains(&program.as_str()))
}

/// Whether an uninstall command may run: an allowlisted program, or the CLI's own `binary`, the program its `verify` step
/// already runs. A vendor's own `self-uninstall` removes what its installer put in place (decision 0164). An install never
/// gets this: it starts an allowlisted program only.
pub fn is_uninstall_program(argv: &[String], binary: &str) -> bool {
    is_vendor_program(argv) || argv.first().is_some_and(|program| program == binary)
}

/// Whether an id or publisher is the first party's: publisher `massorca` or an id `massorca.*`. Only the built-in
/// catalogue may ship these; a package from any other source that claims one is refused (0062).
pub fn reserves_identity(id: &str, publisher: &str) -> bool {
    publisher == RESERVED_PUBLISHER || crate::ids::publisher_of(id) == Some(RESERVED_PUBLISHER)
}

/// An agent CLI the package drives. `kind` points at the agent declaration file, `agents/<id>.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentAdapter {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub cli: Option<AgentCli>,
}

/// How the vendor CLI is obtained. Nothing here is bundled with the package.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCli {
    pub binary: String,
    /// Checked after the install. Absent when the vendor documents no check: the install then succeeds when the binary
    /// resolves.
    #[serde(default)]
    pub verify: Option<Vec<String>>,
    pub source: String,
    /// The day the vendor's install docs were read, `YYYY-MM-DD`, so a stale source is visible.
    #[serde(default, rename = "sourceChecked")]
    pub source_checked: Option<String>,
    #[serde(default)]
    pub install: Vec<CliStep>,
    #[serde(default)]
    pub uninstall: Vec<CliStep>,
}

/// One command for one operating system, as argv.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliStep {
    pub os: CliOs,
    pub command: Vec<String>,
}

/// The operating systems a CLI step may target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliOs {
    Windows,
    Linux,
    Macos,
}

/// An MCP server the core launches and offers to agents.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServer {
    pub id: String,
    pub command: Vec<String>,
}

/// Checks the process-tier contributions: they need a `process` permission, and their ids and files must be sound.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    let adapters = &manifest.contributes.agent_adapters;
    let servers = &manifest.contributes.mcp_servers;
    let has_process = manifest
        .permissions
        .iter()
        .any(|permission| permission.kind == PermissionKind::Process);
    if !has_process && !adapters.is_empty() {
        problems.add(
            "/contributes/agentAdapters",
            "agent adapters need a process permission",
        );
    }
    if !has_process && !servers.is_empty() {
        problems.add(
            "/contributes/mcpServers",
            "MCP servers need a process permission",
        );
    }
    list(
        adapters,
        "agentAdapters",
        "id",
        ids::is_kind_id,
        |adapter| adapter.id.as_str(),
        problems,
        |adapter, here, problems| {
            if RESERVED_AGENT_IDS.contains(&adapter.id.as_str())
                && manifest.publisher != RESERVED_PUBLISHER
            {
                problems.add(
                    at(here, "id"),
                    format!(
                        "`{}` is reserved to publisher {RESERVED_PUBLISHER}",
                        adapter.id
                    ),
                );
            }
            let expected = format!("agents/{}.json", adapter.id);
            if adapter.kind != expected {
                problems.add(at(here, "kind"), format!("must be exactly `{expected}`"));
            }
            if let Some(cli) = &adapter.cli {
                validate_cli(cli, &at(here, "cli"), problems);
            }
        },
    );
    list(
        servers,
        "mcpServers",
        "id",
        ids::is_contribution_id,
        |server| server.id.as_str(),
        problems,
        |server, here, problems| {
            check_argv(&server.command, &at(here, "command"), problems);
        },
    );
}

fn validate_cli(cli: &AgentCli, base: &str, problems: &mut Problems) {
    if !is_bare_program(&cli.binary) {
        problems.add(
            at(base, "binary"),
            "must be a bare program name with no path separators",
        );
    }
    if let Some(verify) = &cli.verify {
        match verify.first() {
            None => problems.add(
                at(base, "verify"),
                "must be a non-empty argv starting with the binary",
            ),
            Some(first) if *first != cli.binary => {
                problems.add(
                    at(base, "verify"),
                    format!("must start with the binary `{}`", cli.binary),
                );
            }
            Some(_) => {}
        }
        check_argv(verify, &at(base, "verify"), problems);
    }
    if let Some(checked) = &cli.source_checked
        && !is_date(checked)
    {
        problems.add(at(base, "sourceChecked"), "must be a date, YYYY-MM-DD");
    }
    if !is_https_url(&cli.source) {
        problems.add(
            at(base, "source"),
            "must be an https URL to the vendor's official install docs",
        );
    }
    check_steps(&cli.install, &at(base, "install"), None, problems);
    check_steps(
        &cli.uninstall,
        &at(base, "uninstall"),
        Some(&cli.binary),
        problems,
    );
}

/// Checks a list of steps. An install starts an allowlisted program; an uninstall, given the CLI's `own_binary`, may start
/// that binary as well (decision 0164).
fn check_steps(steps: &[CliStep], base: &str, own_binary: Option<&str>, problems: &mut Problems) {
    let mut seen = HashSet::new();
    for (index, step) in steps.iter().enumerate() {
        let here = at(base, index);
        if !seen.insert(step.os) {
            problems.add(
                at(&here, "os"),
                "has one entry per operating system in this list",
            );
        }
        check_argv(&step.command, &at(&here, "command"), problems);
        let allowed = match own_binary {
            Some(binary) => is_uninstall_program(&step.command, binary),
            None => is_vendor_program(&step.command),
        };
        if !allowed {
            let programs = VENDOR_PROGRAMS.join(", ");
            let message = match own_binary {
                Some(binary) => format!(
                    "must start with an allowlisted program: {programs}, or the CLI's own binary `{binary}`"
                ),
                None => format!("must start with an allowlisted program: {programs}"),
            };
            problems.add(at(&here, "command"), message);
        }
    }
}

/// A non-empty argv with no control characters, which could hide part of the command in the review.
fn check_argv(argv: &[String], pointer: &str, problems: &mut Problems) {
    if argv.first().is_none_or(|program| program.trim().is_empty()) {
        problems.add(pointer, "must be a non-empty argv");
    }
    if argv.iter().any(|part| part.chars().any(char::is_control)) {
        problems.add(pointer, "must not contain a control character");
    }
    if argv
        .iter()
        .any(|part| part.chars().any(display_chars::is_format))
    {
        problems.add(pointer, "must not contain a format character");
    }
}

/// `YYYY-MM-DD` with a month of 01 to 12 and a day of 01 to 31.
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let number = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .and_then(|part| std::str::from_utf8(part).ok()?.parse::<u32>().ok())
    };
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && number(5..7).is_some_and(|month| (1..=12).contains(&month))
        && number(8..10).is_some_and(|day| (1..=31).contains(&day))
}

/// A program name with no path separator, drive or whitespace, so it is resolved on `PATH` only.
fn is_bare_program(text: &str) -> bool {
    let mut bytes = text.bytes();
    text.len() <= 64
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
}
