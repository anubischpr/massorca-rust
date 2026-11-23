//! The confirm before a finished child is brought back into its parent (X-CH-08). It names the branch the child
//! merges into, offers the merge and the parent's prompt as choices, and sends `agent.bring_back` on its action. A
//! refusal keeps the confirm under the words until the person confirms again or cancels. The service's answer
//! replaces the choices with what it did, and its note in its own words.

use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, WeakEntity, div, px};
use massorca_protocol::methods::agents::{AgentBringBackParams, BringBackResult};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{SPACE_2, SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use crate::model::agent_sessions::{BringBackPrompt, bring_back_prompt, bring_back_refusal};
use crate::service::Refusal;

/// The confirm on screen: its words, the choices it sends, whether its call is on its way, the refusal it last got,
/// and the service's answer once it brought the child back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BringBackConfirm {
    pub(super) prompt: BringBackPrompt,
    /// The session the child comes back to; the answer opens it.
    pub(super) parent: Option<String>,
    /// Whether the child's branch merges into the parent's worktree.
    pub(super) merge: bool,
    /// Whether the parent's run is sent a message about the child.
    pub(super) tell_parent: bool,
    pub(super) sending: bool,
    pub(super) error: Option<String>,
    pub(super) result: Option<BringBackResult>,
}

/// The dialog on screen, if any: the confirm before the call, or the answer once the service made it.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let confirm = shell.bring_back.as_ref()?;
    Some(match &confirm.result {
        Some(result) => result_view(result, weak, cx),
        None => confirm_view(confirm, weak, cx),
    })
}

/// The words of the confirm: the branch the child merges into, or what happens when it is not merged.
pub(super) fn confirm_body(prompt: &BringBackPrompt, merge: bool) -> String {
    if merge {
        prompt.body.clone()
    } else {
        "Nothing is merged. The child is marked as brought back.".to_owned()
    }
}

/// What the answer did, in plain words: whether the branch merged and whether the parent was told. The service's
/// own note follows these lines.
pub(super) fn result_lines(result: &BringBackResult) -> Vec<String> {
    let merge = match (result.merged, &result.merge_commit) {
        (true, Some(commit)) => format!(
            "Merged as commit {}.",
            commit.chars().take(7).collect::<String>()
        ),
        (true, None) => "Merged.".to_owned(),
        (false, _) => "Not merged.".to_owned(),
    };
    let told = if result.prompt_delivered {
        "The parent session was told about it."
    } else {
        "The parent session was not told."
    };
    vec![merge, told.to_owned()]
}

fn confirm_view(confirm: &BringBackConfirm, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let (merge_weak, tell_weak) = (weak.clone(), weak.clone());
    let choices = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            Switch::new("bring-back-merge", confirm.merge)
                .label("Merge the branch")
                .helper("Turn off to bring the child back without merging it.")
                .disabled(confirm.sending)
                .on_change(move |on, _, cx| {
                    let _ = merge_weak.update(cx, |shell, cx| shell.set_bring_back_merge(on, cx));
                }),
        )
        .child(
            Switch::new("bring-back-tell-parent", confirm.tell_parent)
                .label("Tell the parent session")
                .helper("Sends the parent's run a message about the child.")
                .disabled(confirm.sending)
                .on_change(move |on, _, cx| {
                    let _ =
                        tell_weak.update(cx, |shell, cx| shell.set_bring_back_tell_parent(on, cx));
                }),
        );
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(confirm_body(&confirm.prompt, confirm.merge)),
        )
        .child(choices);
    let (ok, dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("bring-back", confirm.prompt.title.clone())
        .open(true)
        .busy(confirm.sending)
        .child(body)
        .action("Bring back")
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = ok.update(cx, |shell, cx| shell.confirm_bring_back(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.cancel_bring_back(cx));
        });
    if let Some(error) = &confirm.error {
        dialog = dialog.error(error.clone());
    }
    dialog.into_any_element()
}

/// The answer: what the merge did, whether the parent was told, then the service's note in its own words. Its action
/// opens the parent session the child came back to.
fn result_view(result: &BringBackResult, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut body = div().flex().flex_col().gap(px(SPACE_2));
    for line in result_lines(result) {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(line),
        );
    }
    if let Some(note) = &result.note {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.primary)
                .child(note.clone()),
        );
    }
    let (open, done) = (weak.clone(), weak.clone());
    Dialog::new("bring-back", "Brought back")
        .open(true)
        .child(body)
        .action("Open parent")
        .cancel_label("Done")
        .on_action(move |_, cx| {
            let _ = open.update(cx, |shell, cx| shell.open_bring_back_parent(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = done.update(cx, |shell, cx| shell.cancel_bring_back(cx));
        })
        .into_any_element()
}

impl Shell {
    /// Opens the confirm for a child under a parent the store holds. Nothing opens for any other session.
    pub(super) fn ask_bring_back(&mut self, child: &str, cx: &mut Context<Self>) {
        let (prompt, parent) = {
            let state = self.store.read(cx).state();
            let Some(prompt) = bring_back_prompt(state, child) else {
                return;
            };
            let parent = state
                .agents
                .iter()
                .find(|session| session.id == child)
                .and_then(|session| session.parent_id.clone());
            (prompt, parent)
        };
        self.bring_back = Some(BringBackConfirm {
            prompt,
            parent,
            merge: true,
            tell_parent: true,
            sending: false,
            error: None,
            result: None,
        });
        cx.notify();
    }

    pub(super) fn cancel_bring_back(&mut self, cx: &mut Context<Self>) {
        self.bring_back = None;
        cx.notify();
    }

    /// Turns the merge choice on or off, unless the call is on its way.
    pub(super) fn set_bring_back_merge(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.bring_back.as_mut().filter(|confirm| !confirm.sending) {
            confirm.merge = on;
            cx.notify();
        }
    }

    /// Turns the choice to tell the parent's run on or off, unless the call is on its way.
    pub(super) fn set_bring_back_tell_parent(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.bring_back.as_mut().filter(|confirm| !confirm.sending) {
            confirm.tell_parent = on;
            cx.notify();
        }
    }

    /// Sends the bring-back the confirm names, with its choices. The confirm stays until the answer: a refusal keeps
    /// it open with its reason, and the answer replaces the choices. The sidebar follows the service's events.
    pub(super) fn confirm_bring_back(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(confirm) = self.bring_back.as_mut() else {
            return;
        };
        if confirm.sending || confirm.result.is_some() {
            return;
        }
        confirm.sending = true;
        confirm.error = None;
        let params = AgentBringBackParams {
            child: confirm.prompt.child.clone(),
            merge: confirm.merge,
            prompt: confirm.tell_parent,
            token: None,
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.agent_bring_back(params).await;
            let _ = this.update(cx, |shell, cx| shell.finish_bring_back(result, cx));
        })
        .detach();
    }

    fn finish_bring_back(
        &mut self,
        result: Result<BringBackResult, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Some(confirm) = self.bring_back.as_mut() {
            confirm.sending = false;
            match result {
                Ok(answer) => {
                    confirm.error = None;
                    confirm.result = Some(answer);
                }
                Err(refusal) => confirm.error = Some(bring_back_refusal(&refusal)),
            }
        }
        cx.notify();
    }

    /// Closes the answer and opens the parent session the child came back to.
    fn open_bring_back_parent(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.bring_back.take().and_then(|confirm| confirm.parent) {
            self.model.open_session(&parent);
        }
        cx.notify();
    }
}
