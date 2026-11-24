//! The dialogs the Marketplace draws over its page: the sign-in of an agent account (D-AC-02) and
//! the confirm before an item is uninstalled (teams-extensions-settings.md 7.4, decision 0059).

use super::{InstallReview, UninstallConfirm};
use crate::model::marketplace::{
    CLI_REMOVAL_SENTENCE, MarketplaceSnapshot, SignInFlow, SignInStep, offers_cli_removal,
    review_lines, uninstall_body_for,
};
use crate::shell::Shell;
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, Styled, WeakEntity, div, px,
};
use massorca_ui::components::account_sign_in_dialog::{AccountSignInDialog, SignInStage};
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::{SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

/// The dialog on screen, if any: the sign-in while an account is being added, the install review of a
/// catalogue package, else the uninstall confirm.
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
    if let Some(review) = &shell.marketplace_review {
        return install_review(review, snapshot, weak, cx);
    }
    let confirm = shell.marketplace_uninstall.as_ref()?;
    uninstall(confirm, snapshot, weak, cx)
}

/// The install review: what the service reported, line by line, a tick for a vendor installer when the review
/// runs one, and Install, which stays off until the review is read and the tick (if any) is set.
fn install_review(
    review: &InstallReview,
    snapshot: &MarketplaceSnapshot,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let item = snapshot.item(review.item)?;
    let theme = *cx.theme();
    let p = &theme.palette;
    let (ok, dismiss, tick) = (weak.clone(), weak.clone(), weak.clone());
    let (lines, runs_installer) = match &review.review {
        Some(answer) => (review_lines(answer), answer.runs_installer),
        None => (Vec::new(), false),
    };
    let may_install =
        review.review.is_some() && !review.sending && (!runs_installer || review.run_installer);
    let mut body = div().flex().flex_col().gap(px(SPACE_2));
    if review.review.is_none() && review.error.is_none() {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child("Reading what installing this package does."),
        );
    }
    for line in lines {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.primary)
                .child(line),
        );
    }
    let mut dialog = Dialog::new(
        "marketplace-install-review",
        format!("Install {}", item.name),
    )
    .open(true)
    .child(body)
    .action("Allow and install")
    .cancel_label("Cancel")
    .busy(review.sending)
    .action_disabled(!may_install)
    .on_action(move |_, cx| {
        let _ = ok.update(cx, |shell, cx| shell.confirm_extension_install(cx));
    })
    .on_dismiss(move |_, cx| {
        let _ = dismiss.update(cx, |shell, cx| shell.cancel_extension_review(cx));
    });
    if runs_installer {
        let tick_box = Checkbox::new("marketplace-install-run-installer", review.run_installer)
            .label("Run the vendor installer listed above")
            .on_change(move |checked, _, cx| {
                let _ = tick.update(cx, |shell, cx| shell.set_install_run_installer(checked, cx));
            });
        dialog = dialog.child(
            div()
                .debug_selector(|| "marketplace-install-run-installer".to_owned())
                .child(tick_box),
        );
    }
    if let Some(error) = &review.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
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
    let (ok, dismiss, tick) = (weak.clone(), weak.clone(), weak.clone());
    let mut dialog = Dialog::new("marketplace-uninstall", format!("Uninstall {}?", item.name))
        .variant(DialogVariant::Destructive)
        .open(true)
        .busy(confirm.sending)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(uninstall_body_for(item, confirm.remove_cli)),
        )
        .action("Uninstall")
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = ok.update(cx, |shell, cx| shell.confirm_marketplace_uninstall(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.cancel_marketplace_uninstall(cx));
        });
    if offers_cli_removal(item) {
        let tick_box = Checkbox::new("marketplace-uninstall-cli-box", confirm.remove_cli)
            .label("Also remove the CLI it installed")
            .on_change(move |checked, _, cx| {
                let _ = tick.update(cx, |shell, cx| {
                    shell.set_marketplace_uninstall_cli(checked, cx)
                });
            });
        dialog = dialog.child(
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .debug_selector(|| "marketplace-uninstall-cli".to_owned())
                        .child(tick_box),
                )
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(theme.palette.text.secondary)
                        .child(CLI_REMOVAL_SENTENCE),
                ),
        );
    }
    if let Some(error) = &confirm.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}
