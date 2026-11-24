//! The protocol's words for its enum values: the serde names the wire uses, so a value a newer peer adds still reads as
//! a word. A value this build cannot name reads as `unknown`.

use serde::Serialize;

/// The wire name of `value`, such as `working` or `type_changed`.
pub(crate) fn word(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::ActivityState;
    use massorca_protocol::methods::git::ChangeKind;

    use super::word;

    #[test]
    fn values_read_as_their_wire_names() {
        assert_eq!(word(ActivityState::Working), "working");
        assert_eq!(word(ChangeKind::TypeChanged), "type_changed");
        assert_eq!(word(ActivityState::Unknown), "unknown");
    }
}
