//! Structured chat runs (decision 0101): a vendor CLI started with piped standard streams instead of a terminal, whose
//! standard output an adapter reads line by line into the session's transcript journal.
//!
//! - [`adapter`]: the [`ChatAdapter`] a kind's dialect implements, and the [`JournalOp`]s it produces.
//! - [`claude`]: the adapter of Claude Code's stream-json dialect.
//! - [`version`]: the probe of a chat kind's CLI version and its supported range.
//! - [`test_adapter`]: the adapter of the fake structured agent's tiny dialect, for MassOrca's own tests.
//! - `journal`: the journal writer and readers over the store (snapshot, replay, history).
//! - `pipe`: the piped child process, its reader, writer and exit.

pub mod adapter;
pub mod claude;
pub(crate) mod journal;
pub(crate) mod pipe;
#[cfg(any(test, feature = "test-hooks"))]
pub mod test_adapter;
pub mod version;

use std::time::Duration;

pub use adapter::{ApprovalAnswer, ChatAdapter, JournalOp, approval_key};
pub use claude::ClaudeAdapter;
use massorca_protocol::methods::chat::ChatBatch;
#[cfg(any(test, feature = "test-hooks"))]
pub use test_adapter::TestAdapter;

/// Most items one session's journal keeps. Past it, the oldest items are dropped as new ones are added: a client pages
/// back only to the oldest kept item, and a cursor whose position predates a dropped change is answered with
/// `reset { too_old }` and a fresh snapshot.
pub const MAX_JOURNAL_ITEMS: u64 = 10_000;
/// Most items a resubscription replays as one batch. A cursor with more changes after it gets `reset { too_old }` and a
/// snapshot instead, which is smaller.
pub const MAX_REPLAY_ITEMS: u32 = 1_000;
/// The newest bytes of a chat run's standard error that are kept, for the exit's evidence (decision 0160).
pub const STDERR_TAIL_BYTES: usize = 16 * 1024;
/// The longest standard output line a chat run reads; a longer line is skipped and noted in the journal.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
/// How long a graceful stop of a chat run waits after closing its standard input before it kills the run's tree; the
/// same grace a terminal run gets (0031).
pub const KILL_GRACE: Duration = Duration::from_secs(5);

/// A journal change of one session, as the stream sends it: the batch carries the epoch and the cursor after it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatChanged {
    pub session: String,
    pub batch: ChatBatch,
}

/// The kinds a test service starts as chat runs with the [`TestAdapter`]: a comma-separated list of kind ids. Read only
/// with the `test-hooks` feature, which only test builds enable.
pub const TEST_CHAT_KINDS_ENV: &str = "MASSORCA_TEST_CHAT_KINDS";

/// Whether `agent.start` of `kind` starts a chat run with the [`TestAdapter`] (test builds only).
#[cfg(feature = "test-hooks")]
pub fn is_test_chat_kind(kind: &str) -> bool {
    std::env::var(TEST_CHAT_KINDS_ENV)
        .is_ok_and(|kinds| kinds.split(',').any(|listed| listed.trim() == kind))
}

/// Names a point where a test service fails a chat start on purpose (`begin`: after the run was let in), so a test can
/// see that a failed start leaves no program behind. Read only with the `test-hooks` feature.
pub const TEST_CHAT_FAIL_AT_ENV: &str = "MASSORCA_TEST_CHAT_FAIL_AT";

/// Whether a test asked the chat start to fail at `point` (test builds only).
#[cfg(feature = "test-hooks")]
pub(crate) fn injected_failure(point: &str) -> bool {
    std::env::var(TEST_CHAT_FAIL_AT_ENV).is_ok_and(|at| at == point)
}

/// Whether a test asked the chat start to fail at `point`: never outside test builds.
#[cfg(not(feature = "test-hooks"))]
pub(crate) fn injected_failure(_point: &str) -> bool {
    false
}
