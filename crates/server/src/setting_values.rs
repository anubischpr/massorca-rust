//! Whether a JSON value fits a setting kind, and the message that says why not.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use crate::names;
use crate::settings::SettingKind;

const MODIFIERS: [&str; 5] = ["Mod", "Ctrl", "Alt", "Shift", "Meta"];

/// `Ok` when `value` fits `kind`; otherwise the reason, worded to follow the setting's key.
pub(crate) fn check(kind: SettingKind, value: &Value) -> Result<(), String> {
    match kind {
        SettingKind::String => expect(value.is_string(), "expects a string"),
        SettingKind::Bool => expect(value.is_boolean(), "expects true or false"),
        SettingKind::Integer => expect(value.is_i64(), "expects a whole number"),
        SettingKind::Path => expect(
            value
                .as_str()
                .is_some_and(|path| Path::new(path).is_absolute()),
            "expects an absolute path",
        ),
        SettingKind::Choice(list) => expect(
            value.as_str().is_some_and(|text| list.contains(&text)),
            &format!("expects one of {}", list.join(", ")),
        ),
        SettingKind::Range { min, max } => expect(
            value
                .as_i64()
                .is_some_and(|number| (min..=max).contains(&number)),
            &format!("expects a whole number from {min} to {max}"),
        ),
        SettingKind::Set(list) => check_set(list, value),
        SettingKind::Colour(list) => expect(
            value
                .as_str()
                .is_some_and(|text| list.contains(&text) || is_hex_colour(text)),
            &format!("expects one of {} or a #rrggbb colour", list.join(", ")),
        ),
        SettingKind::Keybindings => check_keybindings(value),
    }
}

fn expect(acceptable: bool, reason: &str) -> Result<(), String> {
    if acceptable {
        Ok(())
    } else {
        Err(reason.to_owned())
    }
}

fn check_set(list: &[&str], value: &Value) -> Result<(), String> {
    let reason = || format!("expects a list of distinct values from {}", list.join(", "));
    let items = value.as_array().ok_or_else(reason)?;
    let mut seen = HashSet::new();
    for item in items {
        let text = item.as_str().ok_or_else(reason)?;
        if !list.contains(&text) || !seen.insert(text) {
            return Err(reason());
        }
    }
    Ok(())
}

fn is_hex_colour(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn check_keybindings(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("expects an object from action id to a shortcut or null")?;
    let mut taken: HashMap<String, &str> = HashMap::new();
    let mut actions: Vec<&String> = object.keys().collect();
    actions.sort();
    for action in actions {
        if !names::is_valid(action) {
            return Err(format!("has an invalid action id {action:?}"));
        }
        let shortcut = match &object[action] {
            Value::Null => continue,
            Value::String(shortcut) => shortcut,
            _ => return Err(format!("expects a shortcut string or null for {action}")),
        };
        let normal = normalise_shortcut(shortcut)
            .ok_or_else(|| format!("has an invalid shortcut {shortcut:?} for {action}"))?;
        if let Some(other) = taken.insert(normal, action) {
            return Err(format!(
                "binds {shortcut} to both {other} and {action}; unbind one first"
            ));
        }
    }
    Ok(())
}

/// The shortcut with its modifiers in a fixed order and its key in lower case, or `None` when it does not parse.
/// A shortcut is modifiers from [`MODIFIERS`] in any case, each at most once, joined by `+`, then one key: a single visible
/// character other than `+`, or a key name of letters and digits such as `F5`, `Enter` or `Plus`.
fn normalise_shortcut(shortcut: &str) -> Option<String> {
    let mut parts: Vec<&str> = shortcut.split('+').collect();
    let key = parts.pop()?;
    let single = key.chars().count() == 1 && key.chars().all(|c| c.is_ascii_graphic());
    let named = key.len() > 1 && key.len() <= 16 && key.bytes().all(|b| b.is_ascii_alphanumeric());
    if !(single || named) || MODIFIERS.iter().any(|m| m.eq_ignore_ascii_case(key)) {
        return None;
    }
    let mut held = [false; MODIFIERS.len()];
    for part in parts {
        let index = MODIFIERS
            .iter()
            .position(|modifier| modifier.eq_ignore_ascii_case(part))?;
        if std::mem::replace(&mut held[index], true) {
            return None;
        }
    }
    let mut normal: Vec<&str> = MODIFIERS
        .iter()
        .zip(held)
        .filter_map(|(modifier, on)| on.then_some(*modifier))
        .collect();
    let key = key.to_ascii_lowercase();
    normal.push(&key);
    Some(normal.join("+"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{check, normalise_shortcut};
    use crate::settings::SettingKind;

    #[test]
    fn choices_ranges_sets_and_colours() {
        let theme = SettingKind::Choice(&["system", "dark", "light"]);
        assert!(check(theme, &json!("dark")).is_ok());
        assert_eq!(
            check(theme, &json!("Dark")).unwrap_err(),
            "expects one of system, dark, light"
        );

        let depth = SettingKind::Range { min: 1, max: 5 };
        assert!(check(depth, &json!(1)).is_ok());
        assert!(check(depth, &json!(5)).is_ok());
        assert_eq!(
            check(depth, &json!(6)).unwrap_err(),
            "expects a whole number from 1 to 5"
        );
        assert!(check(depth, &json!(2.5)).is_err());

        let items = SettingKind::Set(&["memory", "ports"]);
        assert!(check(items, &json!([])).is_ok());
        assert!(check(items, &json!(["ports", "memory"])).is_ok());
        assert!(check(items, &json!(["ports", "ports"])).is_err());
        assert!(check(items, &json!(["hosts"])).is_err());

        let accent = SettingKind::Colour(&["lavender", "blue"]);
        assert!(check(accent, &json!("blue")).is_ok());
        assert!(check(accent, &json!("#7461D8")).is_ok());
        assert!(check(accent, &json!("#7461d")).is_err());
        assert!(check(accent, &json!("red")).is_err());
    }

    #[test]
    fn shortcuts_parse_in_any_modifier_order() {
        assert_eq!(
            normalise_shortcut("Shift+Mod+P").as_deref(),
            Some("Mod+Shift+p")
        );
        assert_eq!(normalise_shortcut("Ctrl+F5").as_deref(), Some("Ctrl+f5"));
        assert_eq!(normalise_shortcut("Alt+Plus").as_deref(), Some("Alt+plus"));
        assert_eq!(normalise_shortcut("Escape").as_deref(), Some("escape"));
        assert_eq!(normalise_shortcut("Ctrl+Ctrl+P"), None);
        assert_eq!(normalise_shortcut("Ctrl+"), None);
        assert_eq!(normalise_shortcut("Hyper+P"), None);
        assert_eq!(normalise_shortcut("Ctrl+Shift"), None);
        assert_eq!(normalise_shortcut(""), None);
    }

    #[test]
    fn keybindings_refuse_two_actions_on_one_shortcut() {
        let kind = SettingKind::Keybindings;
        assert!(check(kind, &json!({})).is_ok());
        assert!(check(kind, &json!({"app.quick_open": "Mod+P", "notes.pin": null})).is_ok());
        assert_eq!(
            check(
                kind,
                &json!({"app.quick_open": "Mod+P", "notes.pin": "mod+p"})
            )
            .unwrap_err(),
            "binds mod+p to both app.quick_open and notes.pin; unbind one first"
        );
        assert!(check(kind, &json!({"App.Open": "Mod+O"})).is_err());
        assert!(check(kind, &json!({"app.open": 3})).is_err());
        assert!(check(kind, &json!(["Mod+O"])).is_err());
    }
}
