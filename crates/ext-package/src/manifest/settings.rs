//! Settings: typed values with a default, and the section each appears in on the item page.

use serde::Deserialize;

use super::Manifest;
use super::contributes::{list, required};
use crate::error::{Problems, at};
use crate::ids;

/// A typed setting. The default must match the type. `section` decides where it appears on the item page.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub value_type: SettingType,
    #[serde(default)]
    pub section: SettingSection,
    pub default: serde_json::Value,
}

/// The value types a setting may declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingType {
    Boolean,
    Integer,
    Number,
    String,
}

/// Where a setting is shown on the item page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingSection {
    #[default]
    Basic,
    Advanced,
}

/// Checks the settings: keys follow the key rule and are unique, and each default matches its type.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    list(
        &manifest.contributes.settings,
        "settings",
        "key",
        ids::is_setting_key,
        |item| item.key.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
            if !matches_type(item.value_type, &item.default) {
                problems.add(
                    at(here, "default"),
                    format!("must be {}", type_name(item.value_type)),
                );
            }
        },
    );
}

fn matches_type(value_type: SettingType, value: &serde_json::Value) -> bool {
    match value_type {
        SettingType::Boolean => value.is_boolean(),
        SettingType::Integer => value.is_i64() || value.is_u64(),
        SettingType::Number => value.is_number(),
        SettingType::String => value.is_string(),
    }
}

fn type_name(value_type: SettingType) -> &'static str {
    match value_type {
        SettingType::Boolean => "a boolean",
        SettingType::Integer => "an integer",
        SettingType::Number => "a number",
        SettingType::String => "a string",
    }
}
