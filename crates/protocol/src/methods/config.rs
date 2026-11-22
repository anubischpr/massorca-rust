//! Config methods: per-owner settings with defaults. Available once the `config/1` capability is negotiated.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Params: [`ConfigKey`]. Result: [`ConfigValue`].
pub const GET: &str = "config.get";
/// Params: [`ConfigSet`]. Result: [`ConfigValue`].
pub const SET: &str = "config.set";
/// Params: none. Result: [`ConfigList`].
pub const LIST: &str = "config.list";
/// Params: [`ConfigKey`]. Result: [`ConfigValue`], the default that now applies.
pub const RESET: &str = "config.reset";

/// Owner the service assumes when a request leaves `owner` unset.
pub const DEFAULT_OWNER: &str = "core";

/// Params of `config.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub key: String,
}

/// Params of `config.set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub key: String,
    pub value: Value,
}

/// One setting with its default, and whether it still holds that default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigValue {
    pub owner: String,
    pub key: String,
    pub value: Value,
    pub default: Value,
    pub is_default: bool,
    /// The kind: `string`, `bool`, `integer`, `path`, `choice`, `set`, `colour` or `keybindings`. Absent from older services.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The fixed list a `choice`, `set` or `colour` draws from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    /// The inclusive lower bound of a ranged `integer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    /// The inclusive upper bound of a ranged `integer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// Result of `config.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigList {
    pub values: Vec<ConfigValue>,
}
