//! The delegate form's rules without a window: the call a submit sends, the id that call carries and the agent the
//! form starts on. A retry of one submit carries that submit's id, so the service answers with the child it made.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_protocol::methods::agents::AgentDelegateParams;

use super::agent_kinds::KindChoice;

/// Why the form does not send: its prompt is blank.
pub const PROMPT_REQUIRED: &str = "Write what the child should do first.";

/// Why the form does not send: the host offers no agent to start the child on.
pub const NO_KIND: &str = "No agent kind is available on this host.";

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

fn optional(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
