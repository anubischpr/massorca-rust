//! The delegate form's rules without a window: the call a submit sends, the id that call carries, the agent the
//! form starts on, and what the form says about its child: where a writing child runs and whether a read-only child
//! can start. A retry of one submit carries that submit's id, so the service answers with the child it made.

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
    "This workspace has no Git, so the child works in this folder.";

/// What a session that starts no child says when the service gives no reason.
pub const UNAVAILABLE_NOTICE: &str = "This session cannot start a child.";

/// What a session that still starts read-only children says when the service gives no reason.
pub const WRITER_UNAVAILABLE_NOTICE: &str = "This session cannot start a child that writes.";

/// Where a writing child of a session runs, as the service declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritingChild {
    /// A new worktree of the project, so the child's changes stay apart from the parent's.
    OwnWorktree,
    /// The parent's own folder, with no isolation.
    SharedFolder,
    /// No writing child can start under this session.
    Unavailable,
}

/// The `delegation` a session declares: where its writing child runs, whether a read-only child can start, and the
/// reason the service gives when a child cannot start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionDelegation {
    pub writing_child: WritingChild,
    /// Whether a read-only child can start under this session; false means no child can start at all.
    pub read_only_child: bool,
    pub reason: Option<String>,
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

/// Reads `AgentSession.delegation` once crates/protocol carries it; until then no session declares one.
pub fn delegation_of(_session: &AgentSession) -> Option<SessionDelegation> {
    None
}

/// What the form says about its child, and whether it keeps Delegate off. A session with no read-only child starts no
/// child at all, so Delegate stays off and the reason shows, whatever its writing child says. Otherwise a writing child
/// is described unless Read only is on, and an unavailable one keeps Delegate off unless Read only is on.
pub fn placement_notice(
    delegation: &SessionDelegation,
    read_only: bool,
) -> Option<PlacementNotice> {
    if !delegation.read_only_child {
        return Some(PlacementNotice {
            text: reason_or(delegation, UNAVAILABLE_NOTICE),
            blocks_send: true,
        });
    }
    match delegation.writing_child {
        WritingChild::Unavailable => Some(PlacementNotice {
            text: reason_or(delegation, WRITER_UNAVAILABLE_NOTICE),
            blocks_send: !read_only,
        }),
        WritingChild::OwnWorktree | WritingChild::SharedFolder if read_only => None,
        WritingChild::OwnWorktree => Some(notice(OWN_WORKTREE_NOTICE)),
        WritingChild::SharedFolder => Some(notice(SHARED_FOLDER_NOTICE)),
    }
}

/// The reason the service gives, trimmed; without one, the fallback line for what the session cannot start.
fn reason_or(delegation: &SessionDelegation, fallback: &'static str) -> String {
    delegation
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
        .to_owned()
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

    fn declared(
        writing_child: WritingChild,
        read_only_child: bool,
        reason: Option<&str>,
    ) -> SessionDelegation {
        SessionDelegation {
            writing_child,
            read_only_child,
            reason: reason.map(str::to_owned),
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
        let delegation = declared(WritingChild::OwnWorktree, true, None);
        let notice = placement_notice(&delegation, false).expect("described");
        assert_eq!(
            (notice.text.as_str(), notice.blocks_send),
            (
                "The child works in its own copy of the project (a new worktree).",
                false
            )
        );
    }

    #[test]
    fn a_shared_folder_child_is_told_the_workspace_has_no_git() {
        let delegation = declared(WritingChild::SharedFolder, true, None);
        let notice = placement_notice(&delegation, false).expect("described");
        assert_eq!(
            (notice.text.as_str(), notice.blocks_send),
            (
                "This workspace has no Git, so the child works in this folder.",
                false
            )
        );
    }

    #[test]
    fn a_read_only_child_needs_no_isolation_line() {
        for writing_child in [WritingChild::OwnWorktree, WritingChild::SharedFolder] {
            let delegation = declared(writing_child, true, None);
            assert_eq!(placement_notice(&delegation, true), None);
        }
    }

    #[test]
    fn an_unavailable_session_that_allows_read_only_children_sends_them_only_with_read_only_on() {
        let reason = "Writing is paused in this session.";
        let delegation = declared(WritingChild::Unavailable, true, Some(reason));
        let read_only_off = placement_notice(&delegation, false).expect("the reason is shown");
        assert_eq!(
            (read_only_off.text.as_str(), read_only_off.blocks_send),
            (reason, true)
        );
        let read_only_on = placement_notice(&delegation, true).expect("the reason is still shown");
        assert_eq!(
            (read_only_on.text.as_str(), read_only_on.blocks_send),
            (reason, false)
        );
    }

    #[test]
    fn an_unavailable_session_without_read_only_children_keeps_delegate_off_whatever_read_only_says()
     {
        let reason = "This session has reached its delegation depth.";
        let delegation = declared(WritingChild::Unavailable, false, Some(reason));
        for read_only in [false, true] {
            let notice = placement_notice(&delegation, read_only).expect("the reason is shown");
            assert_eq!((notice.text.as_str(), notice.blocks_send), (reason, true));
        }
    }

    #[test]
    fn an_unavailable_session_without_a_reason_says_what_it_can_still_start() {
        for reason in [None, Some("  ")] {
            let writes = declared(WritingChild::Unavailable, true, reason);
            let notice = placement_notice(&writes, false).expect("the fallback is shown");
            assert_eq!(
                (notice.text.as_str(), notice.blocks_send),
                (WRITER_UNAVAILABLE_NOTICE, true)
            );
            let starts_nothing = declared(WritingChild::Unavailable, false, reason);
            let notice = placement_notice(&starts_nothing, false).expect("the fallback is shown");
            assert_eq!(
                (notice.text.as_str(), notice.blocks_send),
                (UNAVAILABLE_NOTICE, true)
            );
        }
    }

    #[test]
    fn a_session_that_starts_no_child_blocks_delegate_for_any_writing_child() {
        let reason = "This session has reached its delegation depth.";
        for writing_child in [
            WritingChild::OwnWorktree,
            WritingChild::SharedFolder,
            WritingChild::Unavailable,
        ] {
            for read_only in [false, true] {
                let with_reason = declared(writing_child, false, Some(reason));
                let notice =
                    placement_notice(&with_reason, read_only).expect("the reason is shown");
                assert_eq!((notice.text.as_str(), notice.blocks_send), (reason, true));
                let without_reason = declared(writing_child, false, None);
                let notice =
                    placement_notice(&without_reason, read_only).expect("the fallback is shown");
                assert_eq!(
                    (notice.text.as_str(), notice.blocks_send),
                    (UNAVAILABLE_NOTICE, true)
                );
            }
        }
    }

    #[test]
    fn a_session_that_declares_no_delegation_gives_the_form_no_line() {
        let session = agent("s", "w1", 10);
        assert_eq!(delegation_of(&session), None);
    }
}
