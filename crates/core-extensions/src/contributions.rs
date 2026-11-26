//! The enabled packages and what their manifests contribute, for the extension points that register them (decision
//! 0153).

use std::path::PathBuf;

use massorca_ext_package::manifest::Manifest;

use crate::catalogue::is_unvouched_first_party;
use crate::error::Error;
use crate::service::Extensions;

/// One enabled installed package: its trusted manifest and the folder its files are in, or why they cannot be trusted.
#[derive(Debug, Clone)]
pub struct EnabledPackage {
    pub id: String,
    pub version: String,
    /// The unpacked folder; contribution paths in the manifest are relative to it.
    pub root: PathBuf,
    /// The manifest, read after the folder's hash matched the lockfile, or the error code that refused it.
    pub manifest: Result<Manifest, String>,
}

impl Extensions {
    /// The enabled packages in lockfile order. A first-party id the catalogue does not vouch for contributes nothing,
    /// as it provides no agent kind (0098).
    pub fn enabled_packages(&self) -> Result<Vec<EnabledPackage>, Error> {
        let lockfile = self.read_lockfile()?;
        Ok(lockfile
            .packages
            .iter()
            .filter(|entry| entry.enabled && !is_unvouched_first_party(entry))
            .map(|entry| EnabledPackage {
                id: entry.id.clone(),
                version: entry.version.clone(),
                root: self.layout.resolve(&entry.path),
                manifest: self
                    .installed_manifest(entry)
                    .map_err(|error| error.code().to_owned()),
            })
            .collect())
    }
}
