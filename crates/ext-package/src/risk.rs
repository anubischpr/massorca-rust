//! Risk, computed by the core from permissions and contributions (`docs/architecture/extensions.md` §6).
//!
//! A manifest cannot set its own risk. Every reason names what raised it, so a review screen can show why.

use crate::manifest::Manifest;
use crate::permission::{Permission, PermissionKind, PermissionScope};

/// The three review levels. The order is the severity order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// One thing that raised the risk above Low.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskReason {
    pub level: RiskLevel,
    /// What raised it, such as `permission:net` or `agentAdapters:acme-agent`.
    pub subject: String,
    /// The review copy shown next to it.
    pub copy: &'static str,
}

/// The computed risk: the highest reason's level, and every reason that raised it above Low.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Risk {
    pub level: RiskLevel,
    pub reasons: Vec<RiskReason>,
}

/// Computes the risk of a manifest.
pub fn assess(manifest: &Manifest) -> Risk {
    let reasons = reasons(manifest);
    let level = reasons
        .iter()
        .map(|reason| reason.level)
        .max()
        .unwrap_or(RiskLevel::Low);
    Risk { level, reasons }
}

pub(crate) fn reasons(manifest: &Manifest) -> Vec<RiskReason> {
    let mut out: Vec<RiskReason> = manifest
        .permissions
        .iter()
        .filter_map(permission_reason)
        .collect();
    let contributes = &manifest.contributes;
    let mut push = |subject: String, copy: &'static str| {
        out.push(RiskReason {
            level: RiskLevel::High,
            subject,
            copy,
        })
    };
    if !contributes.skills.is_empty() {
        push("contribution:skills".to_owned(), INSTRUCTS_AGENTS);
    }
    if !contributes.agent_rules.is_empty() {
        push("contribution:agentRules".to_owned(), INSTRUCTS_AGENTS);
    }
    if !contributes.recipes.is_empty() {
        push("contribution:recipes".to_owned(), RECIPES);
    }
    for adapter in &contributes.agent_adapters {
        push(format!("agentAdapters:{}", adapter.id), PROGRAM);
        if adapter
            .cli
            .as_ref()
            .is_some_and(|cli| !cli.install.is_empty())
        {
            push(
                format!("agentAdapters:{}:install", adapter.id),
                VENDOR_INSTALLER,
            );
        }
    }
    for server in &contributes.mcp_servers {
        push(format!("mcpServers:{}", server.id), PROGRAM);
    }
    out
}

const INSTRUCTS_AGENTS: &str = "Instructs your agents, which act with your rights.";
const RECIPES: &str = "Recipes may run setup commands in your workspaces.";
const PROGRAM: &str = "Runs a program on this computer with your rights.";
const VENDOR_INSTALLER: &str = "Runs the vendor's installer on this computer.";

/// The reason a permission raises the risk, if it does.
pub(crate) fn permission_reason(permission: &Permission) -> Option<RiskReason> {
    let (level, copy) = match permission.kind {
        PermissionKind::Storage
        | PermissionKind::Notifications
        | PermissionKind::WorkspacesRead
        | PermissionKind::AgentsRead => return None,
        PermissionKind::FilesRead | PermissionKind::FilesWrite => {
            let whole_root = match &permission.scope {
                None => true,
                Some(PermissionScope::Files(paths)) => {
                    paths.iter().any(|path| is_whole_root_glob(path))
                }
                Some(_) => false,
            };
            match (whole_root, permission.kind) {
                (true, _) => (
                    RiskLevel::High,
                    "Can read or write every file in the workspace.",
                ),
                (false, PermissionKind::FilesRead) => {
                    (RiskLevel::Medium, "Reads files matching the listed paths.")
                }
                (false, _) => (RiskLevel::Medium, "Writes files matching the listed paths."),
            }
        }
        PermissionKind::Net => {
            let any_host = match &permission.scope {
                None => true,
                Some(PermissionScope::Net(hosts)) => hosts.iter().any(|host| host == "*"),
                Some(_) => false,
            };
            if any_host {
                (RiskLevel::High, "Can send data to any internet host.")
            } else {
                (RiskLevel::Medium, "Sends requests to the listed hosts.")
            }
        }
        PermissionKind::Secrets => (
            RiskLevel::Medium,
            "Uses secrets by name, injected into requests to their hosts.",
        ),
        PermissionKind::TerminalsRead => (
            RiskLevel::Medium,
            "Reads what is shown in workspace terminals.",
        ),
        PermissionKind::TerminalsWrite => (RiskLevel::High, "Types into workspace terminals."),
        PermissionKind::AgentsControl => (RiskLevel::High, "Starts, stops and messages agents."),
        PermissionKind::SessionsRead => (RiskLevel::Medium, "Reads agent conversation history."),
        PermissionKind::Process => (RiskLevel::High, PROGRAM),
        PermissionKind::Credentials => (RiskLevel::High, "Gives agents access to stored sign-ins."),
    };
    Some(RiskReason {
        level,
        subject: format!("permission:{}", permission.kind.wire()),
        copy,
    })
}

/// Whether a glob matches the whole package root: every segment is made only of `*` (`*`, `**`, `*/**`...).
pub(crate) fn is_whole_root_glob(glob: &str) -> bool {
    glob.split('/')
        .all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte == b'*'))
}

#[cfg(test)]
mod tests {
    use super::is_whole_root_glob;

    #[test]
    fn whole_root_globs_are_only_star_segments() {
        assert!(is_whole_root_glob("*"));
        assert!(is_whole_root_glob("**"));
        assert!(is_whole_root_glob("*/**"));
        assert!(!is_whole_root_glob("*.md"));
        assert!(!is_whole_root_glob("src/**"));
    }
}
