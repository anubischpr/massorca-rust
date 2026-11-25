//! Where a kind's CLI keeps its credential files by default, for an import without a folder (decision 0097).
//!
//! Each declared file may name its default folder, per platform, and the vendor's own override variable. Resolution is a
//! pure function of the platform, the user's home folder and an environment lookup, so tests never touch the real
//! home. Detection reads metadata only, never a file's contents.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{CandidateFile, ImportCandidate};

use crate::catalog::Catalog;
use crate::credentials;
use crate::kind::{CredentialSource, CredentialsDecl, KindDecl, remote_path};
use crate::sessions::{AgentError, Agents};

/// The platform a source's folder template is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
    Other,
}

impl Platform {
    /// The platform this process runs on.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// An environment lookup: the value of a variable, or `None` when it is unset.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The folder a declared file is looked for in, and the override variable when that folder came from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub dir: PathBuf,
    pub env: Option<String>,
}

/// One declared file as an import without a folder sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedFile {
    pub name: String,
    pub required: bool,
    /// Where it is looked for; `None` when the kind declares no location for it, or the location cannot be resolved.
    pub path: Option<PathBuf>,
    /// The override variable, when it was set and used.
    pub env: Option<String>,
    /// The import would accept it, judged from its metadata alone.
    pub found: bool,
    /// Why the import would not use a file that is there: `a link`, `a directory`, `larger than 1 MiB`...
    pub reason: Option<String>,
}

/// The value of `name` when it is set, not empty and an absolute path; anything else counts as unset.
fn absolute_var(env: EnvLookup<'_>, name: &str) -> Option<PathBuf> {
    env(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The user's home folder as the vendor CLIs read it: `USERPROFILE` on Windows, `HOME` elsewhere, when it is set to an
/// absolute path.
pub fn user_home(platform: Platform, env: EnvLookup<'_>) -> Option<PathBuf> {
    let name = if platform == Platform::Windows {
        "USERPROFILE"
    } else {
        "HOME"
    };
    absolute_var(env, name)
}

/// The home of this process: [`user_home`] over its own environment, or else the platform's own answer.
pub fn service_home() -> Option<PathBuf> {
    user_home(Platform::current(), &|name| std::env::var_os(name)).or_else(std::env::home_dir)
}

/// A folder template with `{home}` replaced, as an absolute path; `None` when it needs a home and there is none, when it
/// climbs with `..`, or when the result is not absolute on this platform.
pub fn expand(template: &str, home: Option<&Path>) -> Option<PathBuf> {
    let (mut path, rest) = match template.strip_prefix("{home}") {
        Some(rest) => (home?.to_path_buf(), rest),
        None => (PathBuf::from(template), ""),
    };
    for part in rest.split(['/', '\\']).filter(|part| !part.is_empty()) {
        path.push(part);
    }
    let climbs = path
        .components()
        .any(|component| component == Component::ParentDir);
    (!climbs && path.is_absolute()).then_some(path)
}

/// The override variable `name` as a folder: set to a local absolute path that does not climb with `..`. A network or
/// device path (`\\server`, `\\?\`, `//server`) counts as unset, so detection never reaches another machine.
fn override_dir(env: EnvLookup<'_>, name: &str) -> Option<PathBuf> {
    absolute_var(env, name).filter(|dir| {
        !remote_path(&dir.to_string_lossy())
            && !dir
                .components()
                .any(|component| component == Component::ParentDir)
    })
}

/// Where `source` points on `platform`: the override variable's folder when it is set to a usable path (see
/// [`override_dir`]), else the platform's template, else `dir`.
pub fn resolve(
    source: &CredentialSource,
    platform: Platform,
    home: Option<&Path>,
    env: EnvLookup<'_>,
) -> Option<Located> {
    if let Some(name) = &source.env
        && let Some(dir) = override_dir(env, name)
    {
        return Some(Located {
            dir,
            env: Some(name.clone()),
        });
    }
    let template = match platform {
        Platform::Windows => source.windows.as_ref(),
        Platform::Macos => source.macos.as_ref(),
        Platform::Linux => source.linux.as_ref(),
        Platform::Other => None,
    }
    .unwrap_or(&source.dir);
    expand(template, home).map(|dir| Located { dir, env: None })
}

/// Every declared file of `credentials`, where it is looked for and whether the import would accept it there.
pub fn detect(
    credentials: &CredentialsDecl,
    platform: Platform,
    home: Option<&Path>,
    env: EnvLookup<'_>,
) -> Vec<DetectedFile> {
    credentials
        .files
        .iter()
        .map(|name| {
            let located = credentials
                .sources
                .iter()
                .find(|source| &source.file == name)
                .and_then(|source| resolve(source, platform, home, env));
            let path = located.as_ref().map(|located| located.dir.join(name));
            let judged = path.as_deref().map(credentials::judge);
            DetectedFile {
                name: name.clone(),
                required: credentials.required(name),
                found: matches!(judged, Some(Ok(()))),
                reason: match judged {
                    Some(Err(reason)) if reason != credentials::MISSING => Some(reason.to_owned()),
                    _ => None,
                },
                env: located.and_then(|located| located.env),
                path,
            }
        })
        .collect()
}

/// The wire form of a kind's detection: `ready` when every required file was found, `folder` the folder of the first
/// required file.
pub fn candidate(kind: &str, files: &[DetectedFile]) -> ImportCandidate {
    let ready = files
        .iter()
        .filter(|file| file.required)
        .all(|file| file.found)
        && files.iter().any(|file| file.found);
    let folder = files
        .iter()
        .find(|file| file.required)
        .and_then(|file| file.path.as_deref())
        .and_then(Path::parent)
        .map(|dir| dir.display().to_string());
    ImportCandidate {
        kind: kind.to_owned(),
        ready,
        folder,
        files: files
            .iter()
            .map(|file| CandidateFile {
                name: file.name.clone(),
                required: file.required,
                found: file.found,
                path: file.path.as_ref().map(|path| path.display().to_string()),
                env: file.env.clone(),
                reason: file.reason.clone(),
            })
            .collect(),
    }
}

/// [`detect`] for this process: its platform, its home and its environment.
pub fn detect_here(credentials: &CredentialsDecl) -> Vec<DetectedFile> {
    let home = service_home();
    detect(credentials, Platform::current(), home.as_deref(), &|name| {
        std::env::var_os(name)
    })
}

/// The declared credentials of a kind, when it declares files to import.
fn importable(decl: &KindDecl) -> Option<&CredentialsDecl> {
    decl.credentials
        .as_ref()
        .filter(|credentials| !credentials.files.is_empty())
}

impl Agents {
    /// Where an import without a folder would copy each kind's declared files from (decision 0097): one kind, or every
    /// valid, enabled kind that declares credential files. Metadata only; nothing is read or copied.
    pub fn import_candidates(
        &self,
        kind: Option<&str>,
    ) -> Result<Vec<ImportCandidate>, AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        if let Some(kind) = kind {
            let entry = catalog.find(kind).ok_or_else(|| {
                AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {kind}"))
            })?;
            let decl = entry.valid().ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_UNSUPPORTED,
                    format!(
                        "agent kind {kind} is invalid, so it has no usable credentials: {}",
                        entry.problems.join("; ")
                    ),
                )
            })?;
            let credentials = importable(decl).ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_UNSUPPORTED,
                    format!("agent kind {kind} declares no credential files to import"),
                )
            })?;
            return Ok(vec![candidate(&decl.id, &detect_here(credentials))]);
        }
        Ok(catalog
            .entries
            .iter()
            .filter(|entry| entry.enabled())
            .filter_map(|entry| entry.valid())
            .filter_map(|decl| {
                importable(decl).map(|credentials| candidate(&decl.id, &detect_here(credentials)))
            })
            .collect())
    }
}

