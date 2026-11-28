//! Params of the methods that carry a secret value. A decoding error names the field and the expected type only: the
//! serde message would repeat a wrong-typed value, and an error reply reaches the caller and may reach a log.

use std::fmt;

use massorca_protocol::methods::agents::KindSettingsSetParams;
use massorca_protocol::methods::hosted_services::ServiceConnect;
use massorca_protocol::methods::secrets::SecretSet;
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// What a field must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    String,
    Bool,
    /// An array of strings.
    Strings,
    /// An object whose values are strings.
    StringMap,
}

impl Kind {
    fn matches(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Bool => value.is_boolean(),
            Self::Strings => value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string)),
            Self::StringMap => value
                .as_object()
                .is_some_and(|map| map.values().all(Value::is_string)),
        }
    }

    fn expected(self) -> &'static str {
        match self {
            Self::String => "a string",
            Self::Bool => "a boolean",
            Self::Strings => "an array of strings",
            Self::StringMap => "an object of strings",
        }
    }
}

/// The fields of a value-carrying params type: name, kind, and whether it must be present.
pub(crate) trait SecretParams: DeserializeOwned {
    const FIELDS: &'static [(&'static str, Kind, bool)];
}

impl SecretParams for SecretSet {
    const FIELDS: &'static [(&'static str, Kind, bool)] = &[
        ("scope", Kind::String, true),
        ("name", Kind::String, true),
        ("value", Kind::String, true),
        ("replace", Kind::Bool, false),
    ];
}

impl SecretParams for ServiceConnect {
    const FIELDS: &'static [(&'static str, Kind, bool)] = &[
        ("service", Kind::String, true),
        ("token", Kind::String, true),
        ("base_url", Kind::String, false),
        ("email", Kind::String, false),
    ];
}

impl SecretParams for KindSettingsSetParams {
    const FIELDS: &'static [(&'static str, Kind, bool)] = &[
        ("kind", Kind::String, true),
        ("program", Kind::String, false),
        ("args", Kind::Strings, false),
        ("env", Kind::StringMap, false),
        ("remove_env", Kind::Strings, false),
    ];
}

/// Params decoded with redacted errors.
pub(crate) struct Redacted<P>(pub(crate) P);

impl<'de, P: SecretParams> Deserialize<'de> for Redacted<P> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        decode::<P>(value).map(Self).map_err(D::Error::custom)
    }
}

/// A refusal that names a field and a type, never a received value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParamsError(String);

impl fmt::Display for ParamsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn decode<P: SecretParams>(value: Value) -> Result<P, ParamsError> {
    let Value::Object(map) = &value else {
        return Err(ParamsError("params must be an object".to_owned()));
    };
    for &(field, kind, required) in P::FIELDS {
        match map.get(field) {
            None if required => return Err(ParamsError(format!("missing field `{field}`"))),
            None => {}
            Some(found) if !kind.matches(found) => {
                return Err(ParamsError(format!(
                    "invalid type for field `{field}`: expected {}",
                    kind.expected()
                )));
            }
            Some(_) => {}
        }
    }
    serde_json::from_value(value).map_err(|_| ParamsError("invalid params".to_owned()))
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::secrets::SecretSet;
    use serde_json::{Value, json};

    use super::decode;

    const VALUE: &str = "params-value-3c1e";

    fn refused(params: Value) -> String {
        decode::<SecretSet>(params).map(|_| ()).unwrap_err().0
    }

    #[test]
    fn a_wrong_type_names_the_field_and_never_the_value() {
        for (params, message) in [
            (
                json!({ "scope": "global", "name": "A", "value": 7_315_920 }),
                "invalid type for field `value`: expected a string",
            ),
            (
                json!({ "scope": "global", "name": "A", "value": { "nested": VALUE } }),
                "invalid type for field `value`: expected a string",
            ),
            (
                json!({ "scope": "global", "name": "A", "value": "x", "replace": VALUE }),
                "invalid type for field `replace`: expected a boolean",
            ),
            (
                json!({ "scope": "global", "name": "A", "valeu": VALUE }),
                "missing field `value`",
            ),
            (json!([VALUE]), "params must be an object"),
        ] {
            let message_seen = refused(params);
            assert_eq!(message_seen, message);
            assert!(!message_seen.contains(VALUE) && !message_seen.contains("7315920"));
        }
    }

    #[test]
    fn kind_settings_with_a_wrong_typed_value_never_repeat_it() {
        use massorca_protocol::methods::agents::KindSettingsSetParams;
        for (params, message) in [
            (
                json!({ "kind": "claude", "env": { "TOKEN": 7_315_920 } }),
                "invalid type for field `env`: expected an object of strings",
            ),
            (
                json!({ "kind": "claude", "env": [VALUE] }),
                "invalid type for field `env`: expected an object of strings",
            ),
            (
                json!({ "kind": "claude", "args": [VALUE, 1] }),
                "invalid type for field `args`: expected an array of strings",
            ),
            (
                json!({ "kind": "claude", "remove_env": VALUE }),
                "invalid type for field `remove_env`: expected an array of strings",
            ),
        ] {
            let seen = decode::<KindSettingsSetParams>(params)
                .map(|_| ())
                .unwrap_err()
                .0;
            assert_eq!(seen, message);
            assert!(!seen.contains(VALUE) && !seen.contains("7315920"));
        }
        let set: KindSettingsSetParams =
            decode(json!({ "kind": "claude", "env": { "TOKEN": VALUE } })).expect("decodes");
        assert_eq!(set.env["TOKEN"], VALUE);
    }

    #[test]
    fn well_typed_params_decode() {
        let set: SecretSet =
            decode(json!({ "scope": "global", "name": "A", "value": VALUE })).expect("decodes");
        assert_eq!((set.value.as_str(), set.replace), (VALUE, false));
    }
}
