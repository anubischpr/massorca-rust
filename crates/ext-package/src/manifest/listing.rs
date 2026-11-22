//! The `listing` object: the data the Marketplace item page shows, so a client renders any item without item-specific
//! code. The display name and short description stay the top-level `name` and `description`.
//!
//! Category rule: a declared `category` must agree with the contributions it claims (see [`validate`]). When it is
//! absent, [`category`] derives it: `agents` when there is an agent adapter, else `skills` when there are skills, else
//! `themes` when the package holds only theme kinds, else `extensions`. `services` is never derived; it must be
//! declared.

use std::collections::HashSet;

use serde::Deserialize;

use super::contributes::{Contributes, has_non_theme_contributions};
use super::views::Slot;
use super::{Manifest, check_path, is_https_url};
use crate::error::{Problems, at};
use crate::ids;
use crate::permission::PermissionKind;

/// Marketplace data for one item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Listing {
    pub logo: Option<String>,
    pub category: Option<Category>,
    pub developer: Option<Developer>,
    pub mention: Option<String>,
    pub banner: Option<Banner>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

/// The Marketplace categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Extensions,
    Agents,
    Skills,
    Services,
    Themes,
}

/// Who publishes the item as shown to the user. The url, when given, must be https.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Developer {
    pub name: String,
    pub url: Option<String>,
}

/// The banner: an optional image and one to six examples of how to invoke the item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Banner {
    pub image: Option<String>,
    pub examples: Vec<Example>,
}

/// One invocation example. A `mention` example repeats `@` plus the listing's mention, a `command` example names a
/// declared command, and a `usage` example is only a caption.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Example {
    pub kind: ExampleKind,
    pub text: Option<String>,
    pub caption: String,
}

/// The kinds of banner example.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExampleKind {
    Mention,
    Command,
    Usage,
}

/// An account or sign-in the item asks for, shown on the item page under Connection.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub kind: ConnectionKind,
    pub title: String,
    #[serde(default)]
    pub multiple: bool,
}

/// How a connection signs in. `agent-login` is owned by the agents session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ConnectionKind {
    #[serde(rename = "oauth")]
    OAuth,
    #[serde(rename = "api-key")]
    ApiKey,
    #[serde(rename = "agent-login")]
    AgentLogin,
}

/// The category the item page shows: the declared one, else the one derived from the contributions.
pub fn category(manifest: &Manifest) -> Category {
    if let Some(declared) = manifest
        .listing
        .as_ref()
        .and_then(|listing| listing.category)
    {
        return declared;
    }
    derive_category(&manifest.contributes)
}

fn derive_category(contributes: &Contributes) -> Category {
    if !contributes.agent_adapters.is_empty() {
        Category::Agents
    } else if !contributes.skills.is_empty() {
        Category::Skills
    } else if (!contributes.themes.is_empty() || !contributes.icon_themes.is_empty())
        && !has_non_theme_contributions(contributes)
    {
        Category::Themes
    } else {
        Category::Extensions
    }
}

/// The UI slots the package's views are placed in, in declaration order, without repeats.
pub fn placements(manifest: &Manifest) -> Vec<Slot> {
    let mut slots = Vec::new();
    for view in &manifest.contributes.views {
        if !slots.contains(&view.slot) {
            slots.push(view.slot);
        }
    }
    slots
}

/// Checks the listing against the package's contributions and permissions.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    let Some(listing) = &manifest.listing else {
        return;
    };
    if let Some(logo) = &listing.logo {
        check_path(logo, "/listing/logo", problems);
        if !(logo.ends_with(".svg") || logo.ends_with(".png")) {
            problems.add("/listing/logo", "must be a .svg or .png file");
        }
    }
    if let Some(category) = listing.category {
        validate_category(category, &manifest.contributes, problems);
    }
    if let Some(developer) = &listing.developer {
        if developer.name.trim().is_empty() {
            problems.add("/listing/developer/name", "is required");
        }
        if let Some(url) = developer.url.as_deref().filter(|url| !is_https_url(url)) {
            problems.add(
                "/listing/developer/url",
                format!("`{url}` must be an https URL"),
            );
        }
    }
    if let Some(mention) = &listing.mention
        && !is_mention(mention)
    {
        problems.add(
            "/listing/mention",
            "has no @, no spaces, and at most 32 characters",
        );
    }
    if let Some(banner) = &listing.banner {
        validate_banner(banner, listing, manifest, problems);
    }
    validate_connections(&listing.connections, manifest, problems);
}

fn validate_category(category: Category, contributes: &Contributes, problems: &mut Problems) {
    let missing = match category {
        Category::Agents if contributes.agent_adapters.is_empty() => {
            "needs an agentAdapters contribution"
        }
        Category::Skills if contributes.skills.is_empty() => "needs a skills contribution",
        Category::Themes if contributes.themes.is_empty() && contributes.icon_themes.is_empty() => {
            "needs a themes or iconThemes contribution"
        }
        _ => return,
    };
    problems.add(
        "/listing/category",
        format!("`{}` {missing}", category_name(category)),
    );
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

fn validate_banner(
    banner: &Banner,
    listing: &Listing,
    manifest: &Manifest,
    problems: &mut Problems,
) {
    if let Some(image) = &banner.image {
        check_path(image, "/listing/banner/image", problems);
    }
    if !(1..=6).contains(&banner.examples.len()) {
        problems.add("/listing/banner/examples", "needs 1 to 6 examples");
    }
    for (index, example) in banner.examples.iter().enumerate() {
        let here = at("/listing/banner/examples", index);
        if example.caption.trim().is_empty() {
            problems.add(at(&here, "caption"), "is required");
        }
        match example.kind {
            ExampleKind::Mention => validate_mention_example(example, listing, &here, problems),
            ExampleKind::Command => validate_command_example(example, manifest, &here, problems),
            ExampleKind::Usage => {}
        }
    }
}

fn validate_mention_example(
    example: &Example,
    listing: &Listing,
    here: &str,
    problems: &mut Problems,
) {
    let pointer = at(here, "text");
    match (&example.text, &listing.mention) {
        (None, _) => problems.add(pointer, "is required for a mention example"),
        (Some(_), None) => problems.add(pointer, "needs listing.mention"),
        (Some(text), Some(mention)) if *text != format!("@{mention}") => {
            problems.add(pointer, format!("must equal `@{mention}`"));
        }
        _ => {}
    }
}

fn validate_command_example(
    example: &Example,
    manifest: &Manifest,
    here: &str,
    problems: &mut Problems,
) {
    let pointer = at(here, "text");
    let Some(text) = &example.text else {
        problems.add(pointer, "is required for a command example");
        return;
    };
    let declared = manifest
        .contributes
        .commands
        .iter()
        .any(|command| command.id == *text || command.keys.as_deref() == Some(text.as_str()));
    if !declared {
        problems.add(
            pointer,
            format!("`{text}` is not a declared command's keys or id"),
        );
    }
}

fn validate_connections(connections: &[Connection], manifest: &Manifest, problems: &mut Problems) {
    let mut seen = HashSet::new();
    let has = |kind: PermissionKind| {
        manifest
            .permissions
            .iter()
            .any(|permission| permission.kind == kind)
    };
    for (index, connection) in connections.iter().enumerate() {
        let here = at("/listing/connections", index);
        if !ids::is_contribution_id(&connection.id) {
            problems.add(
                at(&here, "id"),
                format!("`{}` is not a valid id", connection.id),
            );
        } else if !seen.insert(connection.id.as_str()) {
            problems.add(
                at(&here, "id"),
                format!("`{}` is declared twice", connection.id),
            );
        }
        if connection.title.trim().is_empty() {
            problems.add(at(&here, "title"), "is required");
        }
        let missing = match connection.kind {
            ConnectionKind::ApiKey if !has(PermissionKind::Secrets) => "needs a secrets permission",
            ConnectionKind::OAuth if !has(PermissionKind::Net) || !has(PermissionKind::Secrets) => {
                "needs net and secrets permissions"
            }
            ConnectionKind::AgentLogin if !has(PermissionKind::Credentials) => {
                "needs a credentials permission"
            }
            ConnectionKind::AgentLogin if manifest.contributes.agent_adapters.is_empty() => {
                "needs at least one agentAdapters entry"
            }
            _ => continue,
        };
        problems.add(here, format!("`{}` {missing}", connection.id));
    }
}

/// A mention without `@`, whitespace or control characters, at most 32 characters.
fn is_mention(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= 32
        && !text.chars().any(|character| {
            character == '@' || character.is_whitespace() || character.is_control()
        })
}
