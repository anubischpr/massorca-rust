//! What `agent.kinds` says about chat sessions (decision 0101, D10), as the desktop holds it. Kept apart from the
//! protocol's `AgentKind` so the start page can read it before the protocol carries the field.

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::AgentKind;

/// Whether a kind can run as a chat, and why not when it cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindChat {
    /// The kind can start a chat now.
    pub supported: bool,
    /// Why not, when `supported` is false.
    pub reason: Option<ChatUnsupportedReason>,
    /// The CLI version the service read, when it read one.
    pub version: Option<String>,
    /// The version range the service supports, for display (for example `>=2.1.0, <2.3.0`).
    pub range: Option<String>,
}

/// Why a kind cannot run as a chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatUnsupportedReason {
    /// The kind's program does not resolve, so no version was read.
    NotDetected,
    /// The program ran but its version could not be read.
    VersionUnknown,
    /// The version read is outside the supported range.
    VersionOutsideRange,
}

/// The chat support of `kind`. `AgentKind` has no `chat` field yet, so every kind reads as no chat; read the field
/// here once the protocol carries it.
pub fn kind_chat(_kind: &AgentKind) -> Option<KindChat> {
    None
}

/// Each kind's chat support by kind id, from the kinds the service lists. A kind without chat is absent.
pub fn kind_chats(kinds: &[AgentKind]) -> BTreeMap<String, KindChat> {
    kinds
        .iter()
        .filter_map(|kind| Some((kind.id.clone(), kind_chat(kind)?)))
        .collect()
}
