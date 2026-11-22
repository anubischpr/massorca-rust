//! Keeps frame dumps out of every log: below `info`, tungstenite prints whole frames, and a `hello` frame carries a
//! pairing secret (decision 0041).

use tracing::Level;
use tracing_subscriber::filter::Targets;

/// Libraries that log frame payloads below `info`.
pub const FRAME_DUMPING_TARGETS: [&str; 2] = ["tungstenite", "tokio_tungstenite"];

/// A filter layer that lets everything through except frame dumps. Add it to every subscriber next to the user's
/// filter, so no `MASSORCA_LOG` value can turn them on.
pub fn frame_dump_guard() -> Targets {
    FRAME_DUMPING_TARGETS.into_iter().fold(
        Targets::new().with_default(Level::TRACE),
        |targets, target| targets.with_target(target, Level::INFO),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allows(target: &str, level: Level) -> bool {
        frame_dump_guard().would_enable(target, &level)
    }

    #[test]
    fn frame_dumps_are_capped_and_everything_else_passes() {
        assert!(!allows("tungstenite::protocol", Level::TRACE));
        assert!(!allows("tokio_tungstenite", Level::DEBUG));
        assert!(allows("tungstenite::protocol", Level::INFO));
        assert!(allows("massorca_server::network", Level::TRACE));
    }
}
