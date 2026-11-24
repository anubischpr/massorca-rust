//! Removes what a crash or a kill during an install, refresh or adoption left under `pkgs/`: staging folders, and
//! version folders that no lockfile entry names.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use massorca_ext_package::ids::is_extension_id;

use crate::error::Error;
use crate::service::Extensions;

const STAGING_PREFIX: &str = ".staging-";

impl Extensions {
    /// Removes the leftovers under `pkgs/` and returns their profile-relative paths. Only call it while no install is
    /// in flight: an install's version folder is unreferenced until its lockfile commit. Nothing is removed when the
    /// lockfile cannot be read. Without a lockfile only staging folders go: a profile copied without it keeps its
    /// packages. A failed removal does not stop the others; the first failure is returned after them.
    pub fn sweep_leftovers(&self) -> Result<Vec<String>, Error> {
        let _changes = self.lock_changes();
        let versions = self.layout.lock_path().exists();
        let lockfile = self.read_lockfile()?;
        let kept: BTreeSet<&str> = lockfile
            .packages
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();
        let pkgs = self.layout.pkgs_dir();
        let packages = match fs::read_dir(&pkgs) {
            Ok(packages) => packages,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(Error::io("cannot list", &pkgs)(error)),
        };
        let mut sweep = Sweep::default();
        for package in packages {
            let package = match package {
                Ok(package) => package,
                Err(error) => {
                    sweep.fail(Error::io("cannot list", &pkgs)(error));
                    continue;
                }
            };
            let Some(id) = package.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            // `DirEntry::file_type` does not follow links, and a Windows junction is not a folder to it.
            if !is_extension_id(&id) || !package.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            sweep.package(&package.path(), &id, versions.then_some(&kept));
        }
        sweep.finish()
    }
}

#[derive(Default)]
struct Sweep {
    removed: Vec<String>,
    first_error: Option<Error>,
}

impl Sweep {
    fn fail(&mut self, error: Error) {
        self.first_error.get_or_insert(error);
    }

    /// Sweeps one `pkgs/<id>/` folder, then removes it when nothing is left in it.
    fn package(&mut self, dir: &Path, id: &str, kept: Option<&BTreeSet<&str>>) {
        let children = match fs::read_dir(dir) {
            Ok(children) => children,
            Err(error) => return self.fail(Error::io("cannot list", dir)(error)),
        };
        for child in children {
            let child = match child {
                Ok(child) => child,
                Err(error) => {
                    self.fail(Error::io("cannot list", dir)(error));
                    continue;
                }
            };
            let Some(name) = child.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let rel = format!("pkgs/{id}/{name}");
            let ours = name.starts_with(STAGING_PREFIX)
                || (is_version_folder(&name)
                    && kept.is_some_and(|kept| !kept.contains(rel.as_str())));
            if !ours || !child.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            // `remove_dir_all` removes links inside the folder without following them.
            match fs::remove_dir_all(child.path()) {
                Ok(()) => self.removed.push(rel),
                Err(error) => self.fail(Error::io("cannot remove", &child.path())(error)),
            }
        }
        let empty = fs::read_dir(dir).is_ok_and(|mut rest| rest.next().is_none());
        if empty {
            match fs::remove_dir(dir) {
                Ok(()) => self.removed.push(format!("pkgs/{id}")),
                Err(error) => self.fail(Error::io("cannot remove", dir)(error)),
            }
        }
    }

    fn finish(self) -> Result<Vec<String>, Error> {
        match self.first_error {
            Some(error) => Err(error),
            None => Ok(self.removed),
        }
    }
}

/// `<version>-<hash8>`, the name `Layout::package_rel_path` gives an unpacked package.
fn is_version_folder(name: &str) -> bool {
    name.rsplit_once('-').is_some_and(|(version, hash8)| {
        !version.is_empty()
            && !version.starts_with('.')
            && hash8.len() == 8
            && hash8.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
