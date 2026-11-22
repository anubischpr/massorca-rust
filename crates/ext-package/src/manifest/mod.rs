//! The `extension.json` manifest: parsing, and every rule a valid manifest must meet.
//!
//! A JSON shape error (wrong type, unknown field, unknown permission kind) stops at the first one, with its path.
//! Every semantic rule is checked in one pass and reported together.

pub mod agent;
pub mod contributes;
pub mod data;
pub mod listing;
pub mod logic;
mod references;
pub mod settings;
pub mod views;

use std::collections::HashSet;
use std::fmt;

use semver::{Version, VersionReq};
use serde::Deserialize;

pub use references::check_files_present;

use crate::error::{Error, Problem, Problems, at};
use crate::ids;
use crate::package_path;
use crate::permission::{self, Permission};
use contributes::Contributes;
use listing::Listing;

/// The manifest file name at the root of every package.
pub const MANIFEST_FILE: &str = "extension.json";

/// A parsed manifest. Parsing checks the JSON shape; [`validate`] checks the rules.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub publisher: String,
    pub description: String,
    pub api: WorldVersion,
    pub requires: Requires,
    #[serde(default)]
    pub views: Option<u32>,
    pub surfaces: Vec<Surface>,
    pub modes: Vec<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub listing: Option<Listing>,
    #[serde(default)]
    pub readme: Option<String>,
}

/// The WIT world version the package needs, `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct WorldVersion {
    pub major: u32,
    pub minor: u32,
}

impl TryFrom<String> for WorldVersion {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let number = |part: &str| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        };
        text.split_once('.')
            .and_then(|(major, minor)| {
                Some(Self {
                    major: number(major)?,
                    minor: number(minor)?,
                })
            })
            .ok_or_else(|| format!("`{text}` is not major.minor"))
    }
}

impl fmt::Display for WorldVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The minimum host version the package needs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    pub massorca: VersionReq,
}

/// Where the package's UI may appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Desktop,
    Android,
}

/// Parses manifest bytes. Only the JSON shape is checked here, see [`validate`] for the rules.
pub fn parse(bytes: &[u8]) -> Result<Manifest, Error> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let manifest = serde_path_to_error::deserialize::<_, Manifest>(&mut de).map_err(shape_error)?;
    de.end()
        .map_err(|error| shape_problem(String::new(), error.to_string()))?;
    Ok(manifest)
}

/// Parses and validates manifest bytes; the result is a manifest that passed every rule.
pub fn load(bytes: &[u8]) -> Result<Manifest, Error> {
    let manifest = parse(bytes)?;
    match validate(&manifest) {
        problems if problems.is_empty() => Ok(manifest),
        problems => Err(Error::InvalidManifest(problems)),
    }
}

/// Every rule a manifest must meet, all reported. An empty list means the manifest is valid.
pub fn validate(manifest: &Manifest) -> Vec<Problem> {
    let mut problems = Problems::default();
    validate_identity(manifest, &mut problems);
    validate_surfaces_modes_views(manifest, &mut problems);
    problems.extend(permission::validate(&manifest.permissions, "/permissions"));
    contributes::validate(manifest, &mut problems);
    listing::validate(manifest, &mut problems);
    problems.into_vec()
}

fn validate_identity(manifest: &Manifest, problems: &mut Problems) {
    if manifest.name.trim().is_empty() {
        problems.add("/name", "is required");
    }
    if manifest.description.trim().is_empty() {
        problems.add("/description", "is required");
    }
    if !ids::is_extension_id(&manifest.id) {
        problems.add(
            "/id",
            "must be publisher.name: two lowercase segments joined by one dot",
        );
    } else if ids::publisher_of(&manifest.id) != Some(manifest.publisher.as_str()) {
        let prefix = ids::publisher_of(&manifest.id).unwrap_or_default();
        problems.add("/publisher", format!("must equal the id prefix `{prefix}`"));
    }
    if let Some(main) = &manifest.main {
        check_path(main, "/main", problems);
        if !(main.ends_with(".wasm") || main.ends_with(".js")) {
            problems.add("/main", "must end in .wasm or .js");
        }
    }
    if let Some(readme) = &manifest.readme {
        check_path(readme, "/readme", problems);
    }
}

fn validate_surfaces_modes_views(manifest: &Manifest, problems: &mut Problems) {
    if manifest.surfaces.is_empty() {
        problems.add("/surfaces", "lists at least one surface");
    }
    let mut surfaces = HashSet::new();
    for (index, surface) in manifest.surfaces.iter().enumerate() {
        if !surfaces.insert(*surface) {
            problems.add(at("/surfaces", index), "is listed twice");
        }
    }
    if manifest.modes.is_empty() {
        problems.add("/modes", "lists at least one mode");
    }
    let mut modes = HashSet::new();
    for (index, mode) in manifest.modes.iter().enumerate() {
        let here = at("/modes", index);
        if !ids::is_contribution_id(mode) {
            problems.add(here, format!("`{mode}` is not a valid mode name"));
        } else if !modes.insert(mode.as_str()) {
            problems.add(here, format!("`{mode}` is listed twice"));
        }
    }
    match manifest.views {
        Some(0) => problems.add("/views", "the view-schema major starts at 1"),
        None if !manifest.contributes.views.is_empty() => {
            problems.add("/views", "is required when views are contributed");
        }
        _ => {}
    }
}

/// Checks that a package path is safe, reporting it at `pointer`.
pub(crate) fn check_path(path: &str, pointer: &str, problems: &mut Problems) {
    if let Err(reason) = package_path::check(path, false) {
        problems.add(
            pointer,
            format!("{reason}: `{path}` is not a package-relative path"),
        );
    }
}

/// `https://` followed by a host, with no user information, whitespace or control characters.
pub(crate) fn is_https_url(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty()
        && !authority.contains('@')
        && text.bytes().all(|byte| byte.is_ascii_graphic())
}

fn shape_error(error: serde_path_to_error::Error<serde_json::Error>) -> Error {
    shape_problem(pointer(error.path()), error.inner().to_string())
}

fn shape_problem(path: String, message: String) -> Error {
    Error::InvalidManifest(vec![Problem { path, message }])
}

/// Converts a serde path such as `contributes.commands[0].id` into a JSON pointer.
fn pointer(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for segment in path.iter() {
        let text = match segment {
            Segment::Seq { index } => index.to_string(),
            Segment::Map { key } | Segment::Enum { variant: key } => {
                key.replace('~', "~0").replace('/', "~1")
            }
            _ => "?".to_owned(),
        };
        out.push('/');
        out.push_str(&text);
    }
    out
}
