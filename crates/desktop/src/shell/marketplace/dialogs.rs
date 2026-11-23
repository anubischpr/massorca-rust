//! The dialogs the Marketplace draws over its page: the sign-in of an agent account (D-AC-02) and
//! the confirm before an item is uninstalled (teams-extensions-settings.md 7.4, decision 0059).

use super::UninstallConfirm;
use crate::model::marketplace::{MarketplaceSnapshot, SignInFlow, SignInStep, uninstall_body};
use crate::shell::Shell;
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, div};
use massorca_ui::components::account_sign_in_dialog::{AccountSignInDialog, SignInStage};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::TYPE_BODY_SM;
use massorca_ui::{ActiveTheme, TextStyled};

/// The dialog on screen, if any: the sign-in while an account is being added, else the confirm.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    if let Some(dialog) = super::sign_in::dialog(shell, weak) {
        return Some(dialog);
    }
    if let Some(dialog) = super::import::dialog(shell, weak, cx) {
        return Some(dialog);
    }
    let snapshot = shell.model().marketplace();
    if let Some(flow) = &snapshot.sign_in {
        return Some(sign_in(shell, weak, snapshot, flow));
    }
    let confirm = shell.marketplace_uninstall.as_ref()?;
    uninstall(confirm, snapshot, weak, cx)
}

/// The account sign-in. Each stage's press runs one step of the source's sign-in.
fn sign_in(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    snapshot: &MarketplaceSnapshot,
    flow: &SignInFlow,
) -> AnyElement {
    let agent = snapshot
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some(flow.agent_kind.as_str()))
        .map_or_else(|| flow.agent_kind.clone(), |item| item.name.clone());
    let (open, submit, retry) = (weak.clone(), weak.clone(), weak.clone());
    let (done, cancel) = (weak.clone(), weak.clone());
    AccountSignInDialog::new("marketplace-sign-in", agent, stage(&flow.step))
        .code_field(shell.marketplace_code.clone())
        .on_open_page(move |_, cx| {
            let _ = open.update(cx, |shell, cx| shell.sign_in_open_page(cx));
        })
        .on_submit(move |window, cx| {
            let _ = submit.update(cx, |shell, cx| shell.sign_in_submit(window, cx));
        })
        .on_retry(move |_, cx| {
            let _ = retry.update(cx, |shell, cx| shell.sign_in_retry(cx));
        })
        .on_done(move |_, cx| {
            let _ = done.update(cx, |shell, cx| shell.end_sign_in(cx));
        })
        .on_cancel(move |_, cx| {
            let _ = cancel.update(cx, |shell, cx| shell.end_sign_in(cx));
        })
        .into_any_element()
}

/// The stage the dialog shows for a step of the sign-in.
fn stage(step: &SignInStep) -> SignInStage {
    match step {
        SignInStep::Starting => SignInStage::Starting,
        SignInStep::WaitingBrowser { url } => SignInStage::WaitingBrowser {
            url: url.clone().into(),
        },
        SignInStep::WaitingCode => SignInStage::WaitingCode,
        SignInStep::Done => SignInStage::Done,
        SignInStep::Failed { reason } => SignInStage::Failed {
            reason: reason.clone().into(),
        },
    }
}

/// The confirm before an uninstall: it names the item and what is removed, and keeps the refusal.
fn uninstall(
    confirm: &UninstallConfirm,
    snapshot: &MarketplaceSnapshot,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let item = snapshot.item(confirm.item)?;
    let theme = *cx.theme();
    let (ok, dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("marketplace-uninstall", format!("Uninstall {}?", item.name))
        .variant(DialogVariant::Destructive)
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(uninstall_body(item)),
        )
        .action("Uninstall")
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = ok.update(cx, |shell, cx| shell.confirm_marketplace_uninstall(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.cancel_marketplace_uninstall(cx));
        });
    if let Some(error) = &confirm.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}
