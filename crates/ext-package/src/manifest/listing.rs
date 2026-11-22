//! The `listing` object: the data the Marketplace item page shows, so a client renders any item without item-specific
//! code. The display name and short description stay the top-level `name` and `description`.
//!
//! Category rule: a declared `category` must agree with the contributions it claims (see [`validate`]). When it is
//! absent, [`category`] derives it: `agents` when there is an agent adapter, else `skills` when there are skills, else
//! `themes` when the package holds only theme kinds, else `extensions`. `services` is never derived; it must be
//! declared.

use serde::Deserialize;

use super::contributes::{Contributes, has_non_theme_contributions};
use super::views::Slot;
use super::{Manifest, check_path, is_https_url};
use crate::error::Problems;

/// Marketplace data for one item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Listing {
    pub logo: Option<String>,
    pub category: Option<Category>,
    pub developer: Option<Developer>,
    pub mention: Option<String>,
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

/// A mention without `@`, whitespace or control characters, at most 32 characters.
fn is_mention(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= 32
        && !text.chars().any(|character| {
            character == '@' || character.is_whitespace() || character.is_control()
        })
}
