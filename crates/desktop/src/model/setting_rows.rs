//! The Settings rows backed by one core setting (SET-01): what each control shows, read from the value the store
//! holds, and the value a change writes through `config.set`. Free of GPUI, so the rules test without a window.

use massorca_protocol::methods::config::ConfigValue;
use serde_json::Value;

/// A control bound to one core setting. Options are `(word the setting stores, label the control shows)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    /// An on-off switch over a `bool` setting.
    Switch(&'static str),
    /// A switch over a `bool` setting with the sound dropdown beside it, which has no backing yet.
    SwitchSound(&'static str),
    /// Segmented options over a `choice` setting, with an action beside them that has no backing yet.
    Choice {
        key: &'static str,
        options: &'static [(&'static str, &'static str)],
        action: Option<&'static str>,
    },
}

impl Live {
    /// The core setting the control reads and writes.
    pub fn key(self) -> &'static str {
        match self {
            Self::Switch(key) | Self::SwitchSound(key) => key,
            Self::Choice { key, .. } => key,
        }
    }
}

/// Whether a switch is on; `None` when the value is not a boolean.
pub fn switch_on(value: &ConfigValue) -> Option<bool> {
    value.value.as_bool()
}

/// The option the value names, by position; `None` when it names none.
pub fn choice_index(options: &[(&str, &str)], value: &ConfigValue) -> Option<usize> {
    let word = value.value.as_str()?;
    options.iter().position(|(option, _)| *option == word)
}

/// The number the value holds; `None` when it is not a whole number.
pub fn number(value: &ConfigValue) -> Option<i64> {
    value.value.as_i64()
}

/// The number one step up or down from the value, kept within the setting's range; `None` when the value is not a
/// number or is already at that end of the range.
pub fn stepped(value: &ConfigValue, step: i64, up: bool) -> Option<i64> {
    let current = number(value)?;
    let moved = if up {
        current.saturating_add(step)
    } else {
        current.saturating_sub(step)
    };
    let low = value.min.unwrap_or(i64::MIN);
    let high = value.max.unwrap_or(i64::MAX);
    let next = moved.clamp(low, high);
    (next != current).then_some(next)
}

/// Whether each option is in the set, in option order; every option reads unchecked when the value is not a list.
pub fn checked(options: &[(&str, &str)], value: &ConfigValue) -> Vec<bool> {
    let words: Vec<&str> = value
        .value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    options
        .iter()
        .map(|(option, _)| words.contains(option))
        .collect()
}

/// The set with option `index` turned on or off, in option order.
pub fn with_option(
    options: &[(&'static str, &str)],
    value: &ConfigValue,
    index: usize,
    on: bool,
) -> Value {
    let mut now = checked(options, value);
    if let Some(slot) = now.get_mut(index) {
        *slot = on;
    }
    Value::Array(
        options
            .iter()
            .zip(now)
            .filter(|(_, on)| *on)
            .map(|((word, _), _)| Value::from(*word))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ITEMS: &[(&str, &str)] = &[("a", "A"), ("b", "B"), ("c", "C")];

    fn held(value: Value) -> ConfigValue {
        ConfigValue {
            owner: "core".into(),
            key: "k".into(),
            value,
            ..Default::default()
        }
    }

    fn ranged(value: i64, min: i64, max: i64) -> ConfigValue {
        ConfigValue {
            min: Some(min),
            max: Some(max),
            ..held(json!(value))
        }
    }

    #[test]
    fn a_switch_reads_only_a_boolean() {
        assert_eq!(switch_on(&held(json!(true))), Some(true));
        assert_eq!(switch_on(&held(json!("true"))), None);
    }

    #[test]
    fn a_choice_reads_the_option_it_names_and_nothing_for_an_unknown_word() {
        assert_eq!(choice_index(ITEMS, &held(json!("b"))), Some(1));
        assert_eq!(choice_index(ITEMS, &held(json!("z"))), None);
        assert_eq!(choice_index(ITEMS, &held(json!(1))), None);
    }

    #[test]
    fn a_step_stays_within_the_range_and_stops_at_its_ends() {
        assert_eq!(stepped(&ranged(4, 1, 16), 1, true), Some(5));
        assert_eq!(stepped(&ranged(100, 90, 130), 5, false), Some(95));
        assert_eq!(stepped(&ranged(128, 90, 130), 5, true), Some(130));
        assert_eq!(stepped(&ranged(16, 1, 16), 1, true), None);
        assert_eq!(stepped(&ranged(1, 1, 16), 1, false), None);
        assert_eq!(stepped(&held(json!("x")), 1, true), None);
    }

    #[test]
    fn a_set_reads_its_words_and_a_toggle_keeps_option_order() {
        let value = held(json!(["c", "a", "z"]));
        assert_eq!(checked(ITEMS, &value), vec![true, false, true]);
        assert_eq!(with_option(ITEMS, &value, 1, true), json!(["a", "b", "c"]));
        assert_eq!(with_option(ITEMS, &value, 0, false), json!(["c"]));
        assert_eq!(checked(ITEMS, &held(json!("a"))), vec![false; 3]);
    }

    #[test]
    fn every_control_names_its_key() {
        assert_eq!(Live::Switch("a.b").key(), "a.b");
    }
}
