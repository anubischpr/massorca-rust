//! The delegate form's rules without a window: the call a submit sends, the id that call carries, the agent the
//! form starts on, and the line that says where a writing child runs. A retry of one submit carries that submit's
//! id, so the service answers with the child it made.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_protocol::methods::agents::{AgentDelegateParams, AgentSession};

use super::agent_kinds::KindChoice;

/// Why the form does not send: its prompt is blank.
pub const PROMPT_REQUIRED: &str = "Write what the child should do first.";

/// Why the form does not send: the host offers no agent to start the child on.
pub const NO_KIND: &str = "No agent kind is available on this host.";

/// What a writing child under its own worktree is told before the person types.
pub const OWN_WORKTREE_NOTICE: &str =
    "The child works in its own copy of the project (a new worktree).";

/// What a writing child in the parent's folder is told before the person types.
pub const SHARED_FOLDER_NOTICE: &str =
    "The child will work in this same folder, not a copy. Its changes are not isolated.";

/// What an unavailable session says when the service gives no reason.
pub const UNAVAILABLE_NOTICE: &str = "This session cannot start a child.";

/// Where a writing child of a session runs, as the service declares it: the `delegation` of an `AgentSession`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WritingChild {
    /// A new worktree of the project, so the child's changes stay apart from the parent's.
    OwnWorktree,
    /// The parent's own folder, with no isolation.
    SharedFolder,
    /// No child can start under this session; `reason` says why, when the service gives one.
    Unavailable { reason: Option<String> },
}

/// The line the Delegate form states before the person types, and whether it keeps Delegate off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementNotice {
    pub text: String,
    pub blocks_send: bool,
}

/// What the delegate form holds when its person sends it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegateDraft {
    /// The session the child starts under.
    pub parent: String,
    pub prompt: String,
    /// The agent the form chose; `None` when the host offers none.
    pub kind: Option<String>,
    /// The workspace name typed; a read-only child takes none.
    pub name: String,
    pub title: String,
    pub read_only: bool,
}

/// The call one submit sends, carrying that submit's `request_id`. The prompt is trimmed, and a blank name or
/// title is left to the service's own default.
pub fn delegate_params(
    draft: &DelegateDraft,
    request_id: &str,
) -> Result<AgentDelegateParams, &'static str> {
    let prompt = draft.prompt.trim();
    if prompt.is_empty() {
        return Err(PROMPT_REQUIRED);
    }
    let Some(kind) = &draft.kind else {
        return Err(NO_KIND);
    };
    Ok(AgentDelegateParams {
        parent: draft.parent.clone(),
        prompt: prompt.to_owned(),
        kind: Some(kind.clone()),
        name: optional(&draft.name).filter(|_| !draft.read_only),
        title: optional(&draft.title),
        read_only: draft.read_only,
        token: None,
        request_id: Some(request_id.to_owned()),
        permission_mode: None,
    })
}

/// A fresh id for one submit, unique within this process and across its restarts.
pub fn new_request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!(
        "{}-{nanos:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// The index of the agent a new form starts on: the parent's, when the host offers it, else the first offered.
pub fn starting_kind(choices: &[KindChoice], parent_kind: &str) -> Option<usize> {
    choices
        .iter()
        .position(|choice| choice.id == parent_kind)
        .or_else(|| (!choices.is_empty()).then_some(0))
}

/// Reads `AgentSession.delegation` once crates/protocol carries it; until then no session declares a placement.
pub fn writing_child_of(_session: &AgentSession) -> Option<WritingChild> {
    None
}

/// What the form says about its child: where a writing child runs, or why none can start. A read-only child needs
/// no isolation, so it gets no line, but an unavailable session still says why and blocks Delegate.
pub fn placement_notice(writing: &WritingChild, read_only: bool) -> Option<PlacementNotice> {
    match writing {
        WritingChild::Unavailable { reason } => Some(PlacementNotice {
            text: reason
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .unwrap_or(UNAVAILABLE_NOTICE)
                .to_owned(),
            blocks_send: true,
        }),
        WritingChild::OwnWorktree | WritingChild::SharedFolder if read_only => None,
        WritingChild::OwnWorktree => Some(notice(OWN_WORKTREE_NOTICE)),
        WritingChild::SharedFolder => Some(notice(SHARED_FOLDER_NOTICE)),
    }
}

fn notice(text: &str) -> PlacementNotice {
    PlacementNotice {
        text: text.to_owned(),
        blocks_send: false,
    }
}

fn optional(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::agent;

    fn draft() -> DelegateDraft {
        DelegateDraft {
            parent: "s".to_owned(),
            prompt: "  Write the tests  ".to_owned(),
            kind: Some("codex".to_owned()),
            name: " catalog-api ".to_owned(),
            title: String::new(),
            read_only: false,
        }
    }

    fn choice(id: &str) -> KindChoice {
        KindChoice {
            id: id.to_owned(),
            name: id.to_owned(),
            unavailable: None,
        }
    }

    #[test]
    fn a_submit_sends_the_trimmed_prompt_the_agent_the_name_and_its_request_id() {
        let params = delegate_params(&draft(), "r1").expect("a prompt and an agent are enough");
        assert_eq!(params.parent, "s");
        assert_eq!(params.prompt, "Write the tests");
        assert_eq!(params.kind.as_deref(), Some("codex"));
        assert_eq!(params.name.as_deref(), Some("catalog-api"));
        assert_eq!((params.title.as_deref(), params.read_only), (None, false));
        assert_eq!(params.token, None);
        assert_eq!(params.request_id.as_deref(), Some("r1"));
    }

    #[test]
    fn a_blank_prompt_is_refused_before_any_call() {
        let blank = DelegateDraft {
            prompt: " \n ".to_owned(),
            ..draft()
        };
        assert_eq!(delegate_params(&blank, "r1"), Err(PROMPT_REQUIRED));
    }

    #[test]
    fn a_read_only_child_takes_no_name_of_its_own() {
        let read_only = DelegateDraft {
            read_only: true,
            ..draft()
        };
        let params = delegate_params(&read_only, "r1").expect("sent");
        assert_eq!((params.read_only, params.name), (true, None));
    }

    #[test]
    fn a_typed_title_is_sent_and_a_blank_one_is_left_to_the_service() {
        let titled = DelegateDraft {
            title: " Feed endpoints ".to_owned(),
            ..draft()
        };
        let sent = delegate_params(&titled, "r1").expect("sent");
        assert_eq!(sent.title.as_deref(), Some("Feed endpoints"));
        let untitled = delegate_params(&draft(), "r1").expect("sent");
        assert_eq!(untitled.title, None);
    }

    #[test]
    fn a_form_with_no_agent_to_start_refuses_to_send() {
        let none = DelegateDraft {
            kind: None,
            ..draft()
        };
        assert_eq!(delegate_params(&none, "r1"), Err(NO_KIND));
    }

    #[test]
    fn the_form_starts_on_the_parents_agent_when_the_host_offers_it() {
        let choices = [choice("claude"), choice("codex")];
        assert_eq!(starting_kind(&choices, "codex"), Some(1));
        assert_eq!(starting_kind(&choices, "gemini"), Some(0));
        assert_eq!(starting_kind(&[], "claude"), None);
    }

    #[test]
    fn each_submit_gets_an_id_of_its_own() {
        assert_ne!(new_request_id(), new_request_id());
    }

    #[test]
    fn an_own_worktree_child_is_told_it_gets_a_copy_of_the_project() {
        let notice = placement_notice(&WritingChild::OwnWorktree, false).expect("described");
        assert_eq!(
            (notice.text.as_str(), notice.blocks_send),
            (
                "The child works in its own copy of the project (a new worktree).",
                false
            )
        );
    }

    #[test]
    fn a_shared_folder_child_is_told_its_changes_are_not_isolated() {
        let notice = placement_notice(&WritingChild::SharedFolder, false).expect("described");
        assert_eq!(
            (notice.text.as_str(), notice.blocks_send),
            (
                "The child will work in this same folder, not a copy. Its changes are not isolated.",
                false
            )
        );
    }

    #[test]
    fn an_unavailable_session_says_why_and_keeps_delegate_off() {
        let reason = "This session has reached its delegation depth.";
        let writing = WritingChild::Unavailable {
            reason: Some(reason.to_owned()),
        };
        let notice = placement_notice(&writing, false).expect("the reason is shown");
        assert_eq!((notice.text.as_str(), notice.blocks_send), (reason, true));
    }

    #[test]
    fn an_unavailable_session_without_a_reason_says_a_child_cannot_start() {
        for reason in [None, Some("  ".to_owned())] {
            let writing = WritingChild::Unavailable { reason };
            let notice = placement_notice(&writing, false).expect("the fallback is shown");
            assert_eq!(
                (notice.text.as_str(), notice.blocks_send),
                (UNAVAILABLE_NOTICE, true)
            );
        }
    }

    #[test]
    fn a_read_only_child_needs_no_isolation_line() {
        assert_eq!(placement_notice(&WritingChild::OwnWorktree, true), None);
        assert_eq!(placement_notice(&WritingChild::SharedFolder, true), None);
    }

    #[test]
    fn an_unavailable_session_blocks_a_read_only_child_too() {
        let writing = WritingChild::Unavailable { reason: None };
        let notice = placement_notice(&writing, true).expect("the reason is still shown");
        assert!(notice.blocks_send);
    }

    #[test]
    fn a_session_that_declares_no_placement_gives_the_form_no_line() {
        let session = agent("s", "w1", 10);
        assert_eq!(writing_child_of(&session), None);
    }
}
