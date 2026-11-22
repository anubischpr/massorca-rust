//! Files a manifest names, and the check that each one is present in the package.

use std::collections::BTreeSet;

use super::Manifest;
use crate::error::{Error, Problem};

/// Every package file the manifest names, each with the pointer that names it. Assumes a validated manifest.
pub(crate) fn referenced(manifest: &Manifest) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(main) = &manifest.main {
        out.push(("/main".to_owned(), main.clone()));
    }
    if let Some(readme) = &manifest.readme {
        out.push(("/readme".to_owned(), readme.clone()));
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
