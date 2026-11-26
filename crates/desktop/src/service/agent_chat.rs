//! What `agent.kinds` says about chat sessions (decision 0101, D10), as the start page reads it. The protocol's
//! `AgentKind.chat` carries it; this module reads that field and re-exports the protocol's types.

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::AgentKind;

pub use massorca_protocol::methods::chat::{ChatUnsupportedReason, KindChat};

/// The chat support of `kind` as the service lists it, or None when the kind lists none.
pub fn kind_chat(kind: &AgentKind) -> Option<KindChat> {
    kind.chat.clone()
}

/// Each kind's chat support by kind id, from the kinds the service lists. A kind without chat is absent.
pub fn kind_chats(kinds: &[AgentKind]) -> BTreeMap<String, KindChat> {
    kinds
        .iter()
        .filter_map(|kind| Some((kind.id.clone(), kind_chat(kind)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::PackageState;

    use super::*;
    use crate::service::fixtures::agent_kind;

    fn listed(supported: bool, reason: Option<ChatUnsupportedReason>) -> KindChat {
        KindChat {
            supported,
            reason,
            ..crate::service::fixtures::base_kind_chat()
        }
    }

    #[test]
    fn a_kind_offers_the_chat_support_the_service_lists_for_it() {
        let mut claude = agent_kind("claude", "Claude", PackageState::Enabled);
        let chat = KindChat {
            version: Some("2.2.0".to_owned()),
            range: Some(">=2.1.0, <2.3.0".to_owned()),
            ..crate::service::fixtures::base_kind_chat()
        };
        claude.chat = Some(chat.clone());

        assert_eq!(kind_chat(&claude), Some(chat));
    }

    #[test]
    fn a_kind_the_service_lists_no_chat_for_has_none() {
        let codex = agent_kind("codex", "Codex", PackageState::Enabled);

        assert_eq!(kind_chat(&codex), None);
    }

    #[test]
    fn the_reason_the_service_gives_is_kept_for_the_tooltip() {
        let mut cursor = agent_kind("cursor", "Cursor", PackageState::Enabled);
        cursor.chat = Some(listed(false, Some(ChatUnsupportedReason::NotDetected)));

        assert_eq!(
            kind_chat(&cursor),
            Some(listed(false, Some(ChatUnsupportedReason::NotDetected)))
        );
    }

    #[test]
    fn kind_chats_keeps_only_the_kinds_that_list_chat() {
        let mut claude = agent_kind("claude", "Claude", PackageState::Enabled);
        claude.chat = Some(listed(true, None));
        let codex = agent_kind("codex", "Codex", PackageState::Enabled);

        let chats = kind_chats(&[claude, codex]);

        assert_eq!(
            chats.keys().map(String::as_str).collect::<Vec<_>>(),
            ["claude"]
        );
        assert_eq!(chats["claude"], listed(true, None));
    }
}
