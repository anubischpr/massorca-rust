//! What a hook event means for a session's activity (decisions 0052, 0053): the values of a kind's `status.events`.

use std::fmt;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// How a turn that is `done` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoneOutcome {
    Success,
    Failure,
    Cancelled,
    Interrupted,
}

impl DoneOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

/// What a hook event means for the session's activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTarget {
    Working,
    Waiting,
    Idle,
    Done(DoneOutcome),
    Ignore,
}

impl EventTarget {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "working" => Self::Working,
            "waiting" => Self::Waiting,
            "idle" => Self::Idle,
            "done" | "done:success" => Self::Done(DoneOutcome::Success),
            "done:failure" => Self::Done(DoneOutcome::Failure),
            "done:cancelled" => Self::Done(DoneOutcome::Cancelled),
            "done:interrupted" => Self::Done(DoneOutcome::Interrupted),
            "ignore" => Self::Ignore,
            _ => return None,
        })
    }
}

impl fmt::Display for EventTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Working => f.write_str("working"),
            Self::Waiting => f.write_str("waiting"),
            Self::Idle => f.write_str("idle"),
            Self::Done(DoneOutcome::Success) => f.write_str("done"),
            Self::Done(outcome) => write!(f, "done:{}", outcome.as_str()),
            Self::Ignore => f.write_str("ignore"),
        }
    }
}

impl Serialize for EventTarget {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EventTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            de::Error::custom(format!(
                "unknown event target {text:?}: expected working, waiting, idle, done, done:<success|failure|cancelled|interrupted> or ignore"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn event_targets_parse_every_form() {
        let events: BTreeMap<String, EventTarget> = serde_json::from_value(serde_json::json!({
            "a": "working", "b": "waiting", "c": "idle", "d": "done",
            "e": "done:failure", "f": "done:cancelled", "g": "done:interrupted", "h": "ignore"
        }))
        .expect("parses");
        assert_eq!(events["a"], EventTarget::Working);
        assert_eq!(events["d"], EventTarget::Done(DoneOutcome::Success));
        assert_eq!(events["e"], EventTarget::Done(DoneOutcome::Failure));
        assert_eq!(events["h"], EventTarget::Ignore);
        assert_eq!(events["g"].to_string(), "done:interrupted");
        assert_eq!(events["d"].to_string(), "done");
        assert_eq!(serde_json::to_value(events["f"]).unwrap(), "done:cancelled");
    }

    #[test]
    fn an_unknown_target_names_itself_in_the_error() {
        let error = serde_json::from_str::<EventTarget>("\"done:maybe\"").expect_err("refused");
        assert!(error.to_string().contains("done:maybe"), "{error}");
    }
}
