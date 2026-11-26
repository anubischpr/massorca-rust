//! The dialogs the Marketplace draws over its page: the sign-in of an agent account (D-AC-02) and
//! the confirm before an item is uninstalled (teams-extensions-settings.md 7.4, decision 0059).

use super::{InstallReview, RollbackConfirm, RunLogView, UninstallConfirm};
use crate::model::marketplace::{
    CLI_REMOVAL_SENTENCE, MarketplaceSnapshot, ReviewOutcome, RunRow, SignInFlow, SignInStep,
    offers_cli_removal, remove_details, remove_outcome, review_details, review_outcome,
    rollback_details, rollback_outcome, run_log_text, run_row,
};
use crate::shell::Shell;
use gpui::{
    AnyElement, App, ClipboardItem, InteractiveElement, IntoElement, ParentElement, Styled,
    WeakEntity, div, px,
};
use massorca_ui::components::account_sign_in_dialog::{AccountSignInDialog, SignInStage};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY, TYPE_BODY_SM, TYPE_CAPTION, TYPE_MONO_SM};
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
    if let Some(dialog) = super::add_folder::dialog(shell, weak, cx) {
        return Some(dialog);
    }
    if let Some(dialog) = super::services::dialog(shell, weak, cx) {
        return Some(dialog);
    }
    let snapshot = shell.model().marketplace();
    if let Some(flow) = &snapshot.sign_in {
        return Some(sign_in(shell, weak, snapshot, flow));
    }
    if let Some(review) = &shell.marketplace_review {
        return install_review(review, snapshot, weak, cx);
    }
    if let Some(view) = &shell.marketplace_run_log {
        return Some(run_log(view, weak, cx));
    }
    if let Some(confirm) = &shell.marketplace_rollback {
        return Some(rollback(confirm, weak, cx));
    }
    let confirm = shell.marketplace_uninstall.as_ref()?;
    uninstall(confirm, snapshot, weak, cx)
}

/// The install review: only what the button will do (owner ruling 2026-11-24), a tick for a vendor installer when the
/// review runs one, and the button, which stays off until the review is read and the tick (if any) is set.
fn install_review(
    review: &InstallReview,
    snapshot: &MarketplaceSnapshot,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let name = review_name(review, snapshot)?;
    let theme = *cx.theme();
    let p = &theme.palette;
    let (ok, dismiss, tick) = (weak.clone(), weak.clone(), weak.clone());
    let outcome = review
        .review
        .as_ref()
        .map(|answer| review_outcome(answer, &name));
    let runs_installer = review
        .review
        .as_ref()
        .is_some_and(|answer| answer.runs_installer);
    let may_install =
        review.review.is_some() && !review.sending && (!runs_installer || review.run_installer);
    let (title, action_label) = outcome.as_ref().map_or_else(
        || (format!("Install {name}"), "Add to MassOrca"),
        |outcome| (outcome.title().to_owned(), outcome.button()),
    );
    let sentence = |text: String| {
        div()
            .text_style(TYPE_BODY)
            .text_color(p.text.primary)
            .child(text)
    };
    let mut body = div().flex().flex_col().gap(px(SPACE_1));
    body = match outcome {
        None if review.error.is_none() => body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child("Reading what installing this package does."),
        ),
        None => body,
        Some(ReviewOutcome::Add { body: text, .. }) => body.child(sentence(text)),
        Some(ReviewOutcome::Install {
            sentence: text,
            command,
            ..
        }) => body.child(sentence(text)).child(
            div()
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.primary)
                .child(command),
        ),
    };
    if let Some(answer) = &review.review {
        let (toggle, open) = (weak.clone(), review.details_open);
        body = body.child(details(
            "marketplace-install-details",
            open,
            review_details(answer),
            move |cx| {
                let _ = toggle.update(cx, |shell, cx| shell.set_install_details(!open, cx));
            },
            cx,
        ));
    }
    let mut dialog = Dialog::new("marketplace-install-review", title)
        .open(true)
        .child(body)
        .action(action_label)
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
        // Copy details gives the whole refusal, with what was looked for and where, not only the line shown.
        let text = error.clone();
        dialog = dialog.child(
            Button::new("marketplace-install-copy", "Copy details")
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                }),
        );
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}

/// Show details: a ghost toggle, collapsed by default, and the technical lines under it when open.
fn details(
    id: &'static str,
    open: bool,
    lines: Vec<String>,
    on_toggle: impl Fn(&mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let label = if open { "Hide details" } else { "Show details" };
    let mut column = div().flex().flex_col().gap(px(SPACE_1)).child(
        Button::new(id, label)
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, cx| on_toggle(cx)),
    );
    if open {
        let mut list = div()
            .debug_selector(move || format!("{id}-list"))
            .flex()
            .flex_col()
            .gap(px(SPACE_1));
        for line in lines {
            list = list.child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(theme.palette.text.secondary)
                    .child(line),
            );
        }
        column = column.child(list);
    }
    column.into_any_element()
}

/// The name the review says: the catalogue item's display name, or the name the review reports for a folder package.
fn review_name(review: &InstallReview, snapshot: &MarketplaceSnapshot) -> Option<String> {
    Some(match (review.item, &review.review) {
        (Some(item), _) => snapshot.item(item)?.name.clone(),
        (None, Some(answer)) => answer.name.clone(),
        (None, None) => "from a folder".to_owned(),
    })
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

/// The run log: each run on one line, its output under Show details when it wrote any, and Copy details for the whole log.
fn run_log(view: &RunLogView, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let (close, dismiss) = (weak.clone(), weak.clone());
    let mut body = div().flex().flex_col().gap(px(SPACE_2));
    match &view.runs {
        None => {
            body = body.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.secondary)
                    .child("Reading the run log."),
            );
        }
        Some(Err(sentence)) => {
            body = body.child(
                div()
                    .text_style(TYPE_BODY)
                    .text_color(theme.palette.text.primary)
                    .child(sentence.clone()),
            );
        }
        Some(Ok(runs)) if runs.is_empty() => {
            body = body.child(
                div()
                    .debug_selector(|| "marketplace-run-log-empty".to_owned())
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.secondary)
                    .child("No runs recorded yet."),
            );
        }
        Some(Ok(runs)) => {
            for (index, run) in runs.iter().enumerate() {
                let open = view.open_output.contains(&index);
                body = body.child(run_entry(index, &run_row(run), open, weak, cx));
            }
            let text = run_log_text(runs);
            body = body.child(
                Button::new("marketplace-run-log-copy", "Copy details")
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                    }),
            );
        }
    }
    Dialog::new("marketplace-run-log", format!("{} log", view.name))
        .open(true)
        .child(body)
        .action("Close")
        .on_action(move |_, cx| {
            let _ = close.update(cx, |shell, cx| shell.close_run_log(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_run_log(cx));
        })
        .into_any_element()
}

/// One run of the log: its line, and a ghost Show details toggle for its output when it wrote any.
fn run_entry(
    index: usize,
    row: &RunRow,
    open: bool,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let mut entry = div().flex().flex_col().gap(px(SPACE_1)).child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.primary)
            .child(row.summary()),
    );
    if !row.output.is_empty() {
        let (toggle, label) = (
            weak.clone(),
            if open { "Hide details" } else { "Show details" },
        );
        entry = entry.child(
            Button::new(format!("marketplace-run-{index}-output"), label)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, cx| {
                    let _ =
                        toggle.update(cx, |shell, cx| shell.set_run_log_output(index, !open, cx));
                }),
        );
        if open {
            for line in &row.output {
                entry = entry.child(
                    div()
                        .text_style(TYPE_MONO_SM)
                        .text_color(theme.palette.text.secondary)
                        .child(line.clone()),
                );
            }
        }
    }
    entry.into_any_element()
}

/// The confirm before a rollback: one sentence on what it puts back, and the versions and consent under Show details.
fn rollback(confirm: &RollbackConfirm, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let outcome = rollback_outcome(&confirm.name, &confirm.version);
    let (ok, dismiss, toggle) = (weak.clone(), weak.clone(), weak.clone());
    let (open, lines) = (
        confirm.details_open,
        rollback_details(&confirm.from, &confirm.version, &confirm.consent),
    );
    let mut dialog = Dialog::new("marketplace-rollback", outcome.title)
        .open(true)
        .busy(confirm.sending)
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(theme.palette.text.primary)
                .child(outcome.sentence),
        )
        .child(details(
            "marketplace-rollback-details",
            open,
            lines,
            move |cx| {
                let _ = toggle.update(cx, |shell, cx| shell.set_rollback_details(!open, cx));
            },
            cx,
        ))
        .action(outcome.button)
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = ok.update(cx, |shell, cx| shell.confirm_marketplace_rollback(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.cancel_marketplace_rollback(cx));
        });
    if let Some(error) = &confirm.error {
        dialog = dialog.error(error.clone());
    }
    dialog.into_any_element()
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
    let outcome = remove_outcome(item, confirm.remove_cli);
    let (toggle, open) = (weak.clone(), confirm.details_open);
    let mut dialog = Dialog::new("marketplace-uninstall", outcome.title)
        .variant(DialogVariant::Destructive)
        .open(true)
        .busy(confirm.sending)
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(theme.palette.text.primary)
                .child(outcome.sentence),
        )
        .action(outcome.button)
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
    dialog = dialog.child(details(
        "marketplace-uninstall-details",
        open,
        remove_details(item, confirm.remove_cli),
        move |cx| {
            let _ = toggle.update(cx, |shell, cx| shell.set_uninstall_details(!open, cx));
        },
        cx,
    ));
    if let Some(error) = &confirm.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}
