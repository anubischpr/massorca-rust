//! `extensions.points` (capability `extension-points/1`, decision 0153): what each owner registered on the tool, skill,
//! view, theme and status-rule points. A debug read; fields are only ever added (0002).

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`ExtensionPointsList`]. Brings the points in line with the enabled packages first.
pub const POINTS: &str = "extensions.points";

/// Result of `extensions.points`: owners by wire form (`core` or `ext:<id>`), and the packages that registered nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPointsList {
    #[serde(default)]
    pub owners: Vec<ExtensionPointOwner>,
    #[serde(default)]
    pub problems: Vec<ExtensionPointProblem>,
}

/// The keys one owner holds on each point; a key is `<package id>/<contribution id>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPointOwner {
    pub owner: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub views: Vec<String>,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub status_rules: Vec<String>,
}

/// An enabled package whose contributions are not registered, such as `extension.tampered` or
/// `extension.point_refused`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPointProblem {
    pub id: String,
    pub code: String,
    pub message: String,
}
