//! Version probing of a chat kind's CLI (decision 0101, D10; inventory AG-03). The service runs `<program> --version`
//! once per installed program (the probe cache of [`crate::probe`]), reads the first `MAJOR.MINOR.PATCH` it prints, and
//! compares it with the range the kind's `chat` block declares. A version outside the range is refused rather than tried:
//! a changed frame vocabulary would be misread.

use std::path::Path;
use std::time::Duration;

use massorca_protocol::methods::chat::{ChatUnsupportedReason, KindChat};

use crate::account_check::RunOutput;
use crate::check_retry::CheckRunner;
use crate::kind::{ChatDecl, KindDecl};

/// The arguments of the version probe.
pub const VERSION_ARGS: &[&str] = &["--version"];

/// Programs a test service may probe: paths joined as `PATH` joins them. Any other program is not run, and its kind
/// reads as `version_unknown`, so no test ever runs a vendor CLI it happens to find. Read only in test builds.
pub const TEST_PROBE_ALLOW_ENV: &str = "MASSORCA_TEST_VERSION_PROBE_ALLOW";

/// A `MAJOR.MINOR.PATCH` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// `text` when it is exactly `MAJOR.MINOR.PATCH`, each part decimal digits.
    pub fn parse_exact(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let mut next = || {
            parts
                .next()
                .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|part| part.parse().ok())
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        parts.next().is_none().then_some(version)
    }

    /// The first word of `output` that starts with `MAJOR.MINOR.PATCH` (an optional `v` before it, and a pre-release or
    /// build suffix after it, are allowed), as `claude --version` prints `2.1.330 (Claude Code)`.
    pub fn find_in(output: &str) -> Option<Self> {
        output.split_whitespace().find_map(|word| {
            let word = word.strip_prefix('v').unwrap_or(word);
            let end = word
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(word.len());
            Self::parse_exact(word[..end].trim_end_matches('.'))
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The declared range as `agent.kinds` shows it, for example `>=2.1.258, <3.0.0`.
pub fn range_text(chat: &ChatDecl) -> String {
    format!(
        ">={}, <{}.0.0",
        chat.min_version,
        chat.max_major.saturating_add(1)
    )
}

/// Whether `version` is in the declared range: not below `min_version`, and of a major version not above `max_major`.
pub fn in_range(version: Version, chat: &ChatDecl) -> bool {
    Version::parse_exact(&chat.min_version).is_some_and(|min| version >= min)
        && version.major <= chat.max_major
}

/// The chat support a probe shows: `None` for `probe` means the kind's program was not found. A probe that failed or
/// printed no version is `version_unknown`.
pub(crate) fn support(chat: &ChatDecl, probe: Option<&RunOutput>) -> KindChat {
    let range = Some(range_text(chat));
    let Some(probe) = probe else {
        return KindChat {
            supported: false,
            reason: Some(ChatUnsupportedReason::NotDetected),
            version: None,
            range,
        };
    };
    let version = (probe.exit == Some(0))
        .then(|| Version::find_in(&probe.output))
        .flatten();
    let Some(version) = version else {
        return KindChat {
            supported: false,
            reason: Some(ChatUnsupportedReason::VersionUnknown),
            version: None,
            range,
        };
    };
    let supported = in_range(version, chat);
    KindChat {
        supported,
        reason: (!supported).then_some(ChatUnsupportedReason::VersionOutsideRange),
        version: Some(version.to_string()),
        range,
    }
}

/// The chat support of `decl` with its resolved `program` (`None` when none resolves), probing for at most `timeout`
/// with `runner` standing in for the program when it is set: `None` for a kind without a `chat` block. The probe is
/// cached per program path and modified time; `fresh` probes whatever is cached. The flag says the probe was stopped at
/// its time limit: the support then reads `version_unknown`, but it is no answer yet, and it is cached marked as a
/// timeout for [`crate::probe::FAILED_PROBE_TTL`] (decision 0059, amended 2026-11-27).
pub(crate) fn kind_chat_with(
    decl: &KindDecl,
    program: Option<&Path>,
    timeout: Duration,
    runner: Option<&CheckRunner>,
    fresh: bool,
) -> Option<(KindChat, bool)> {
    let chat = decl.chat.as_ref()?;
    let Some(program) = program else {
        return Some((support(chat, None), false));
    };
    if runner.is_none() && !probe_allowed(program) {
        let not_run = RunOutput {
            exit: None,
            output: String::new(),
        };
        return Some((support(chat, Some(&not_run)), false));
    }
    let args: Vec<String> = VERSION_ARGS.iter().map(|&arg| arg.to_owned()).collect();
    let probed = crate::probe::probe_with(decl, program, &args, timeout, runner, fresh);
    Some((support(chat, Some(&probed.run)), probed.timed_out))
}

/// Whether a test build may run `program`: only a program [`TEST_PROBE_ALLOW_ENV`] lists.
#[cfg(any(test, feature = "test-hooks"))]
fn probe_allowed(program: &Path) -> bool {
    std::env::var_os(TEST_PROBE_ALLOW_ENV)
        .is_some_and(|allowed| std::env::split_paths(&allowed).any(|path| path == program))
}

/// A release build probes every program it resolves.
#[cfg(not(any(test, feature = "test-hooks")))]
fn probe_allowed(_program: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude_range() -> ChatDecl {
        ChatDecl {
            adapter: "claude-stream-json".to_owned(),
            min_version: "2.1.258".to_owned(),
            max_major: 2,
        }
    }

    fn printed(exit: Option<i32>, output: &str) -> RunOutput {
        RunOutput {
            exit,
            output: output.to_owned(),
        }
    }

    #[test]
    fn a_version_is_read_from_what_the_cli_prints() {
        let version = |text| Version::find_in(text).map(|version| version.to_string());
        assert_eq!(version("2.1.330 (Claude Code)").as_deref(), Some("2.1.330"));
        assert_eq!(version("claude v2.2.0-beta.1\n").as_deref(), Some("2.2.0"));
        assert_eq!(
            version("Claude Code 10.20.30.").as_deref(),
            Some("10.20.30")
        );
        assert_eq!(version("no version here"), None);
        assert_eq!(version("2.1 (Claude Code)"), None);
        assert_eq!(Version::parse_exact("2.1.0.1"), None);
        assert_eq!(Version::parse_exact("2.x.0"), None);
    }

    #[test]
    fn a_version_inside_the_range_is_supported() {
        for text in ["2.1.258 (Claude Code)", "2.1.330 (Claude Code)", "2.9.0"] {
            let chat = support(&claude_range(), Some(&printed(Some(0), text)));
            assert!(chat.supported, "{text}: {chat:?}");
            assert_eq!(chat.reason, None);
            assert_eq!(chat.range.as_deref(), Some(">=2.1.258, <3.0.0"));
        }
        let chat = support(
            &claude_range(),
            Some(&printed(Some(0), "2.1.330 (Claude Code)")),
        );
        assert_eq!(chat.version.as_deref(), Some("2.1.330"));
    }

    #[test]
    fn a_version_outside_the_range_is_refused_with_the_version_it_read() {
        for text in ["2.1.257", "1.0.128", "3.0.0"] {
            let chat = support(&claude_range(), Some(&printed(Some(0), text)));
            assert!(!chat.supported, "{text}");
            assert_eq!(
                chat.reason,
                Some(ChatUnsupportedReason::VersionOutsideRange)
            );
            assert_eq!(chat.version.as_deref(), Some(text));
        }
    }

    #[test]
    fn an_unreadable_or_failed_probe_is_version_unknown_and_a_missing_program_not_detected() {
        for probe in [
            printed(Some(0), "Claude Code"),
            printed(Some(1), "2.1.330"),
            printed(None, ""),
        ] {
            let chat = support(&claude_range(), Some(&probe));
            assert!(!chat.supported);
            assert_eq!(chat.reason, Some(ChatUnsupportedReason::VersionUnknown));
            assert_eq!(chat.version, None);
        }
        let chat = support(&claude_range(), None);
        assert_eq!(chat.reason, Some(ChatUnsupportedReason::NotDetected));
    }

    #[test]
    fn a_test_build_never_runs_a_program_it_was_not_given() {
        let kind = crate::catalog::built_ins()
            .into_iter()
            .find(|kind| kind.id == "claude")
            .expect("built in");
        let missing = std::env::temp_dir().join("massorca-no-such-claude-for-a-version-probe.exe");
        let timeout = crate::probe::PROBE_TIMEOUT;
        let (chat, timed_out) =
            kind_chat_with(&kind, Some(&missing), timeout, None, false).expect("a chat block");
        assert_eq!(chat.reason, Some(ChatUnsupportedReason::VersionUnknown));
        assert!(!timed_out, "a program that is not run did not time out");
        assert_eq!(
            kind_chat_with(&kind, None, timeout, None, false).and_then(|(chat, _)| chat.reason),
            Some(ChatUnsupportedReason::NotDetected)
        );
    }
}
