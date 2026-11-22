//! The contribution kinds a package declares. Each kind lives in its own module: data kinds in `data`, logic kinds in
//! `logic`, and the process tier in `agent`.

use serde::Deserialize;

/// Every contribution a package declares. Missing kinds are empty; unknown kinds are rejected.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
pub struct Contributes {}
