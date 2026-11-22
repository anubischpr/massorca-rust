//! The confirm before a finished child is brought back into its parent (X-CH-08). It names the branch the
//! child merges into, sends `agent.bring_back` on its action, and keeps a refusal under the words until the
//! person confirms again or cancels.

use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, WeakEntity, div};
use massorca_protocol::methods::agents::{AgentBringBackParams, BringBackResult};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::tokens::TYPE_BODY_SM;
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use crate::model::agent_sessions::{BringBackPrompt, bring_back_prompt, bring_back_refusal};
use crate::service::Refusal;

/// The confirm on screen: its words, whether its call is on its way, and the refusal it last got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BringBackConfirm {
    pub(super) prompt: BringBackPrompt,
    pub(super) sending: bool,
    pub(super) error: Option<String>,
}

/// The dialog on screen, if any.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let confirm = shell.bring_back.as_ref()?;
    let theme = *cx.theme();
    let (ok, dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("bring-back", confirm.prompt.title.clone())
        .open(true)
        .busy(confirm.sending)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(confirm.prompt.body.clone()),
        )
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
    Some(dialog.into_any_element())
}

impl Shell {
    /// Opens the confirm for a child under a parent the store holds. Nothing opens for any other session.
    pub(super) fn ask_bring_back(&mut self, child: &str, cx: &mut Context<Self>) {
        if let Some(prompt) = bring_back_prompt(self.store.read(cx).state(), child) {
            self.bring_back = Some(BringBackConfirm {
                prompt,
                sending: false,
                error: None,
            });
            cx.notify();
        }
    }

    pub(super) fn cancel_bring_back(&mut self, cx: &mut Context<Self>) {
        self.bring_back = None;
        cx.notify();
    }

    /// Sends the bring-back the confirm names. The confirm stays until the answer: a refusal keeps it open
    /// with its reason, and success closes it; the sidebar follows the service's events, not this answer.
    pub(super) fn confirm_bring_back(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(confirm) = self.bring_back.as_mut() else {
            return;
        };
        if confirm.sending {
            return;
        }
        confirm.sending = true;
        confirm.error = None;
        let params = AgentBringBackParams {
            child: confirm.prompt.child.clone(),
            merge: true,
            prompt: true,
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
        match result {
            Ok(_) => self.bring_back = None,
            Err(refusal) => {
                if let Some(confirm) = self.bring_back.as_mut() {
                    confirm.sending = false;
                    confirm.error = Some(bring_back_refusal(&refusal));
                }
            }
        }
        cx.notify();
    }
}
