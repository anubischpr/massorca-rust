//! Serializable views of the package format's types, for the review, the item page and the agents API.

use massorca_ext_package::manifest::Manifest;
use massorca_ext_package::manifest::agent::{AgentCli, CliStep};
use massorca_ext_package::manifest::listing::{
    self, Category, ConnectionKind, ExampleKind, Listing,
};
use massorca_ext_package::permission::{Permission, PermissionScope};
use massorca_ext_package::risk::{Risk, RiskLevel};
use serde::Serialize;

use crate::host::os_name;

/// One declared permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PermissionView {
    pub kind: &'static str,
    pub reason: String,
    /// The scope entries; `None` means "any".
    pub scope: Option<Vec<String>>,
}

impl PermissionView {
    pub fn of(permission: &Permission) -> Self {
        Self {
            kind: permission.kind.wire(),
            reason: permission.reason.clone(),
            scope: permission.scope.as_ref().map(|scope| match scope {
                PermissionScope::Files(entries)
                | PermissionScope::Net(entries)
                | PermissionScope::Secrets(entries)
                | PermissionScope::Process(entries)
                | PermissionScope::Credentials(entries) => entries.clone(),
            }),
        }
    }
}

/// The computed risk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskView {
    pub level: &'static str,
    pub reasons: Vec<RiskReasonView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskReasonView {
    pub level: &'static str,
    pub subject: String,
    pub copy: &'static str,
}

impl RiskView {
    pub fn of(risk: &Risk) -> Self {
        Self {
            level: level_name(risk.level),
            reasons: risk
                .reasons
                .iter()
                .map(|reason| RiskReasonView {
                    level: level_name(reason.level),
                    subject: reason.subject.clone(),
                    copy: reason.copy,
                })
                .collect(),
        }
    }
}

fn level_name(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

/// The Marketplace page data of `listing`, with the category resolved (declared or derived).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListingView {
    pub logo: Option<String>,
    pub category: &'static str,
    pub developer: Option<DeveloperView>,
    pub mention: Option<String>,
    pub banner: Option<BannerView>,
    pub connections: Vec<ConnectionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeveloperView {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BannerView {
    pub image: Option<String>,
    pub examples: Vec<ExampleView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExampleView {
    pub kind: &'static str,
    pub text: Option<String>,
    pub caption: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionView {
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    pub multiple: bool,
}

impl ListingView {
    pub fn of(manifest: &Manifest) -> Self {
        let empty = Listing {
            logo: None,
            category: None,
            developer: None,
            mention: None,
            banner: None,
            connections: Vec::new(),
            website: None,
            privacy: None,
            terms: None,
        };
        let listing = manifest.listing.as_ref().unwrap_or(&empty);
        Self {
            logo: listing.logo.clone(),
            category: category_name(listing::category(manifest)),
            developer: listing.developer.as_ref().map(|developer| DeveloperView {
                name: developer.name.clone(),
                url: developer.url.clone(),
            }),
            mention: listing.mention.clone(),
            banner: listing.banner.as_ref().map(|banner| BannerView {
                image: banner.image.clone(),
                examples: banner
                    .examples
                    .iter()
                    .map(|example| ExampleView {
                        kind: match example.kind {
                            ExampleKind::Mention => "mention",
                            ExampleKind::Command => "command",
                            ExampleKind::Usage => "usage",
                        },
                        text: example.text.clone(),
                        caption: example.caption.clone(),
                    })
                    .collect(),
                colors: banner.colors.clone(),
            }),
            connections: listing
                .connections
                .iter()
                .map(|connection| ConnectionView {
                    id: connection.id.clone(),
                    kind: match connection.kind {
                        ConnectionKind::OAuth => "oauth",
                        ConnectionKind::ApiKey => "api-key",
                        ConnectionKind::AgentLogin => "agent-login",
                    },
                    title: connection.title.clone(),
                    multiple: connection.multiple,
                })
                .collect(),
            website: listing.website.clone(),
            privacy: listing.privacy.clone(),
            terms: listing.terms.clone(),
        }
    }

    /// The package paths of the images the listing names: the logo and the banner image.
    pub fn images(&self) -> impl Iterator<Item = &str> {
        self.logo
            .iter()
            .chain(
                self.banner
                    .as_ref()
                    .and_then(|banner| banner.image.as_ref()),
            )
            .map(String::as_str)
    }
}

fn category_name(category: Category) -> &'static str {
    match category {
        Category::Extensions => "extensions",
        Category::Agents => "agents",
        Category::Skills => "skills",
        Category::Services => "services",
        Category::Themes => "themes",
        Category::Tools => "tools",
    }
}

/// An agent adapter's `cli` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliSpec {
    pub binary: String,
    /// `None` when the vendor documents no check; the install then needs only the binary to resolve.
    pub verify: Option<Vec<String>>,
    pub source: String,
    pub source_checked: Option<String>,
    pub install: Vec<CliStepView>,
    pub uninstall: Vec<CliStepView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliStepView {
    pub os: &'static str,
    pub command: Vec<String>,
}

impl CliSpec {
    pub fn of(cli: &AgentCli) -> Self {
        let steps = |steps: &[CliStep]| {
            steps
                .iter()
                .map(|step| CliStepView {
                    os: os_name(step.os),
                    command: step.command.clone(),
                })
                .collect()
        };
        Self {
            binary: cli.binary.clone(),
            verify: cli.verify.clone(),
            source: cli.source.clone(),
            source_checked: cli.source_checked.clone(),
            install: steps(&cli.install),
            uninstall: steps(&cli.uninstall),
        }
    }
}

/// An agent kind's declaration as the review shows it: what it starts and what it reads or strips, so the person sees
/// more than the package hash. Read leniently: a declaration that is not JSON is shown as unreadable, and core-agents
/// refuses it when it loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindView {
    pub id: String,
    pub readable: bool,
    /// The programs the kind may start, in the order it tries them.
    pub program: Vec<String>,
    pub args: Vec<String>,
    /// Environment variable names the kind sets, reads or strips. Values are not shown.
    pub env: Vec<String>,
    /// The variable that points at the credential home, and the files kept there.
    pub credential_home_env: Option<String>,
    /// A built-in keeps its own credential home: the review says so and names no variable (0062, V3).
    pub own_credential_home: bool,
    pub credential_files: Vec<String>,
}

impl KindView {
    /// `builtin` marks a built-in kind, whose internal names the review leaves out.
    pub fn of(id: &str, declaration: Option<&[u8]>, builtin: bool) -> Self {
        let value: Option<serde_json::Value> =
            declaration.and_then(|bytes| serde_json::from_slice(bytes).ok());
        let Some(value) = value else {
            return Self {
                id: id.to_owned(),
                readable: false,
                program: Vec::new(),
                args: Vec::new(),
                env: Vec::new(),
                credential_home_env: None,
                own_credential_home: false,
                credential_files: Vec::new(),
            };
        };
        let strings = |key: &str| -> Vec<String> {
            value[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        };
        let credentials = &value["credentials"];
        // Only the keys core-agents accepts: `credentials.strip_env` names the variables a run removes.
        let mut env: Vec<String> = credentials["strip_env"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|name| name.as_str().map(str::to_owned))
            .collect();
        env.sort();
        let home = credentials["home_env"].as_str().map(str::to_owned);
        Self {
            id: id.to_owned(),
            readable: true,
            program: strings("program"),
            args: strings("args"),
            env: if builtin { Vec::new() } else { env },
            credential_home_env: if builtin { None } else { home.clone() },
            own_credential_home: builtin && home.is_some(),
            credential_files: credentials["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|file| file.as_str().map(str::to_owned))
                .collect(),
        }
    }
}

/// An MCP server the package offers to agents, with the command the core starts for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpView {
    pub id: String,
    pub command: Vec<String>,
}

impl McpView {
    pub fn of(manifest: &Manifest) -> Vec<Self> {
        manifest
            .contributes
            .mcp_servers
            .iter()
            .map(|server| Self {
                id: server.id.clone(),
                command: server.command.clone(),
            })
            .collect()
    }
}
