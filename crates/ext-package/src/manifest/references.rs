//! Files a manifest names, and the check that each one is present in the package.

use std::collections::BTreeSet;

use super::Manifest;
use crate::error::{Error, Problem, at};

/// Every package file the manifest names, each with the pointer that names it. Assumes a validated manifest.
pub(crate) fn referenced(manifest: &Manifest) -> Vec<(String, String)> {
    let contributes = &manifest.contributes;
    let mut out = Vec::new();
    if let Some(main) = &manifest.main {
        out.push(("/main".to_owned(), main.clone()));
    }
    if let Some(readme) = &manifest.readme {
        out.push(("/readme".to_owned(), readme.clone()));
    }
    if let Some(logo) = manifest
        .listing
        .as_ref()
        .and_then(|listing| listing.logo.as_ref())
    {
        out.push(("/listing/logo".to_owned(), logo.clone()));
    }
    for (index, theme) in contributes.themes.iter().enumerate() {
        let here = format!("/contributes/themes/{index}");
        out.push((at(&here, "document"), theme.document.clone()));
    }
    for (index, theme) in contributes.icon_themes.iter().enumerate() {
        let here = format!("/contributes/iconThemes/{index}");
        out.push((at(&here, "document"), theme.document.clone()));
    }
    for (kind, items) in [
        ("locales", &contributes.locales),
        ("recipes", &contributes.recipes),
        ("agentRules", &contributes.agent_rules),
    ] {
        for (index, item) in items.iter().enumerate() {
            let here = format!("/contributes/{kind}/{index}");
            out.push((at(&here, "document"), item.document.clone()));
        }
    }
    for (index, skill) in contributes.skills.iter().enumerate() {
        let here = format!("/contributes/skills/{index}");
        out.push((at(&here, "path"), format!("{}/SKILL.md", skill.path)));
    }
    for (index, view) in contributes.views.iter().enumerate() {
        let here = format!("/contributes/views/{index}");
        out.push((at(&here, "document"), view.document.clone()));
    }
    for (index, tool) in contributes.tools.iter().enumerate() {
        let here = format!("/contributes/tools/{index}");
        out.push((at(&here, "input"), tool.input.clone()));
    }
    for (index, adapter) in contributes.agent_adapters.iter().enumerate() {
        let here = format!("/contributes/agentAdapters/{index}");
        out.push((at(&here, "kind"), adapter.kind.clone()));
    }
    out
}

/// Checks that every file the manifest names is in `files`, which holds the package's file paths.
pub fn check_files_present<'a>(
    manifest: &Manifest,
    files: impl IntoIterator<Item = &'a str>,
) -> Result<(), Error> {
    let present: BTreeSet<&str> = files.into_iter().collect();
    let missing: Vec<Problem> = referenced(manifest)
        .into_iter()
        .filter(|(_, path)| !present.contains(path.as_str()))
        .map(|(path, file)| Problem {
            path,
            message: format!("`{file}` is not in the package"),
        })
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::MissingFiles(missing))
    }
}
