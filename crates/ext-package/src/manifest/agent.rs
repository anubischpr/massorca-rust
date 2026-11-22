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
    pub verify: Vec<String>,
    pub source: String,
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
    match cli.verify.first() {
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
    check_argv(&cli.verify, &at(base, "verify"), problems);
    if !is_https_url(&cli.source) {
        problems.add(
            at(base, "source"),
            "must be an https URL to the vendor's official install docs",
        );
    }
    check_steps(&cli.install, &at(base, "install"), problems);
    check_steps(&cli.uninstall, &at(base, "uninstall"), problems);
}

fn check_steps(steps: &[CliStep], base: &str, problems: &mut Problems) {
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
