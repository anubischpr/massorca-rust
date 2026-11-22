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
    }
}

/// An agent adapter's `cli` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliSpec {
    pub binary: String,
    pub verify: Vec<String>,
    pub source: String,
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
            install: steps(&cli.install),
            uninstall: steps(&cli.uninstall),
        }
    }
}
