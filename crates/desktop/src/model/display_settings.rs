//! The settings the window draws with (SET-01), read from the settings store: the Status bar's shown items, the text
//! preferences, the row density and whether the Session summary is offered. A value that is missing, of the wrong type or out of range keeps its default, so one damaged key never
//! hides the others.

use massorca_ui::components::list_row::RowDensity;
use massorca_ui::text_preferences::{
    SCROLLBACK_MAX, SCROLLBACK_MIN, SIZE_PERCENT_MAX, SIZE_PERCENT_MIN, TextPreferences,
};
use serde_json::Value;

use super::status_items::StatusItem;

const STATUS_BAR_KEY: &str = "appearance.status_bar";
const TEXT_SIZE_KEY: &str = "appearance.text_size";
const INTERFACE_FONT_KEY: &str = "appearance.font.interface";
const MONOSPACE_FONT_KEY: &str = "appearance.font.monospace";
const TERMINAL_FONT_KEY: &str = "terminal.font";
const SCROLLBACK_KEY: &str = "terminal.scrollback";
const DENSITY_KEY: &str = "appearance.density";
const SESSION_SUMMARY_KEY: &str = "appearance.session_summary";

/// The settings the window draws with.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplaySettings {
    /// The Status bar items the settings show, in bar order.
    pub status_items: Vec<StatusItem>,
    /// The text size, the families and the terminal scrollback.
    pub text: TextPreferences,
    /// The height one-line rows take.
    pub density: RowDensity,
    /// Whether the Session summary region and its Title bar toggle are offered.
    pub session_summary: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            status_items: StatusItem::ALL.to_vec(),
            text: TextPreferences::default(),
            density: RowDensity::Default,
            session_summary: true,
        }
    }
}

impl DisplaySettings {
    /// The settings `value` reads, where `value(key)` is the value the store holds for `key`, if it holds one.
    pub fn from_lookup<'a>(value: impl Fn(&str) -> Option<&'a Value>) -> Self {
        let defaults = TextPreferences::default();
        Self {
            status_items: value(STATUS_BAR_KEY)
                .and_then(status_items)
                .unwrap_or_else(|| StatusItem::ALL.to_vec()),
            text: TextPreferences {
                size_percent: value(TEXT_SIZE_KEY)
                    .and_then(size_percent)
                    .unwrap_or(defaults.size_percent),
                interface_family: family(value(INTERFACE_FONT_KEY), defaults.interface_family),
                monospace_family: family(value(MONOSPACE_FONT_KEY), defaults.monospace_family),
                terminal_family: family(value(TERMINAL_FONT_KEY), defaults.terminal_family),
                scrollback_lines: value(SCROLLBACK_KEY)
                    .and_then(scrollback_lines)
                    .unwrap_or(defaults.scrollback_lines),
            },
            density: value(DENSITY_KEY)
                .and_then(Value::as_str)
                .and_then(density)
                .unwrap_or(RowDensity::Default),
            session_summary: value(SESSION_SUMMARY_KEY)
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

/// The density a setting word names.
fn density(word: &str) -> Option<RowDensity> {
    match word {
        "compact" => Some(RowDensity::Compact),
        "default" => Some(RowDensity::Default),
        "comfortable" => Some(RowDensity::Comfortable),
        _ => None,
    }
}

/// The items the list names, in bar order; a word that names no item is ignored.
fn status_items(value: &Value) -> Option<Vec<StatusItem>> {
    let words = value.as_array()?;
    Some(
        StatusItem::ALL
            .into_iter()
            .filter(|item| {
                words
                    .iter()
                    .any(|word| word.as_str() == Some(setting_word(*item)))
            })
            .collect(),
    )
}

/// The word the settings use for a Status bar item.
fn setting_word(item: StatusItem) -> &'static str {
    match item {
        StatusItem::Agents => "agent_status",
        StatusItem::Memory => "memory",
        StatusItem::Terminals => "terminals",
        StatusItem::Ports => "ports",
        StatusItem::Hosts => "hosts",
    }
}

fn size_percent(value: &Value) -> Option<u16> {
    let percent = u16::try_from(value.as_u64()?).ok()?;
    (SIZE_PERCENT_MIN..=SIZE_PERCENT_MAX)
        .contains(&percent)
        .then_some(percent)
}

fn scrollback_lines(value: &Value) -> Option<usize> {
    let rows = usize::try_from(value.as_u64()?).ok()?;
    (SCROLLBACK_MIN..=SCROLLBACK_MAX)
        .contains(&rows)
        .then_some(rows)
}

/// The family the value names, or `default` when it names none.
fn family<T: for<'a> From<&'a str>>(value: Option<&Value>, default: T) -> T {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or(default, T::from)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    fn settings(entries: &[(&str, Value)]) -> DisplaySettings {
        let stored: HashMap<&str, Value> = entries.iter().cloned().collect();
        DisplaySettings::from_lookup(|key| stored.get(key))
    }

    #[test]
    fn nothing_stored_gives_the_defaults() {
        assert_eq!(settings(&[]), DisplaySettings::default());
    }

    #[test]
    fn the_status_bar_shows_the_listed_items_in_bar_order() {
        let shown =
            settings(&[(STATUS_BAR_KEY, json!(["hosts", "agent_status", "nope"]))]).status_items;
        assert_eq!(shown, vec![StatusItem::Agents, StatusItem::Hosts]);
        assert!(
            settings(&[(STATUS_BAR_KEY, json!([]))])
                .status_items
                .is_empty()
        );
    }

    #[test]
    fn values_in_range_apply() {
        let applied = settings(&[
            (TEXT_SIZE_KEY, json!(120)),
            (INTERFACE_FONT_KEY, json!(" Inter ")),
            (TERMINAL_FONT_KEY, json!("Fira Code")),
            (SCROLLBACK_KEY, json!(5000)),
        ])
        .text;
        assert_eq!(applied.size_percent, 120);
        assert_eq!(applied.interface_family.as_ref(), "Inter");
        assert_eq!(applied.terminal_family.as_ref(), "Fira Code");
        assert_eq!(applied.scrollback_lines, 5000);
    }

    #[test]
    fn values_out_of_range_or_of_the_wrong_type_keep_the_defaults() {
        let kept = settings(&[
            (TEXT_SIZE_KEY, json!(250)),
            (MONOSPACE_FONT_KEY, json!("")),
            (TERMINAL_FONT_KEY, json!(7)),
            (SCROLLBACK_KEY, json!("many")),
            (STATUS_BAR_KEY, json!("hosts")),
        ]);
        assert_eq!(kept, DisplaySettings::default());
    }
}
