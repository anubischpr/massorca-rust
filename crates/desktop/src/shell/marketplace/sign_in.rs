//! Add account on a connected service (decision 0059): `agent.accounts.add` starts the sign-in in a service
//! terminal, which the dialog shows and the person types into. The service keeps or discards the account when the
//! terminal ends, and the dialog follows the kind's list to say which.

use gpui::prelude::*;
use gpui::{AnyElement, App, AsyncApp, Context, Entity, Task, WeakEntity, div};
use massorca_protocol::methods::agents::AccountsAddParams;
use massorca_ui::components::account_sign_in_dialog::{AccountSignInDialog, SignInStage};
use massorca_ui::components::terminal_surface::TerminalSurface;

use crate::model::marketplace::{AccountId, TerminalSignIn, terminal_sign_in};
use crate::model::work_panel::{NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS};
use crate::service::{AttachRole, Refusal, ServiceHandle, StreamEnd, TerminalStreamItem};
use crate::shell::Shell;
use crate::shell::work_panel::{Step, apply_update, wait_until_connected};

/// The reason the dialog gives when the service did not keep the account.
const DISCARDED: &str = "The sign-in ended without signing in, so the account was not kept.";

/// A sign-in this desktop started, from the Add account press until the dialog closes.
pub(in crate::shell) struct SignIn {
    kind: String,
    phase: Phase,
}

enum Phase {
    /// `agent.accounts.add` has not answered yet.
    Starting,
    /// The service refused to start the sign-in; the sentence says why.
    Refused(String),
    Terminal(SignInTerminal),
}

/// The service terminal that runs a sign-in, as the dialog shows it.
struct SignInTerminal {
    account: AccountId,
    terminal: String,
    surface: Entity<TerminalSurface>,
    _follow: Task<()>,
    /// The terminal has ended, so the service decides the account.
    ended: bool,
}

impl SignIn {
    /// The account whose sign-in terminal still runs, which is busy until it ends.
    pub(super) fn running_account(&self) -> Option<AccountId> {
        match &self.phase {
            Phase::Terminal(view) if !view.ended => Some(view.account),
            _ => None,
        }
    }
}

impl Shell {
    /// Starts a sign-in for `kind` on the service, with the dialog showing that it is starting.
    pub(super) fn start_service_sign_in(&mut self, kind: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        self.marketplace_accounts.sign_in = Some(SignIn {
            kind: kind.to_owned(),
            phase: Phase::Starting,
        });
        let params = AccountsAddParams {
            kind: kind.to_owned(),
            label: None,
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.accounts_add(params).await;
            let _ = this.update(cx, |shell, cx| match result {
                Ok(added) => shell.show_sign_in_terminal(&added.account.id, added.terminal_id, cx),
                Err(refusal) => shell.set_sign_in_phase(Phase::Refused(refusal.sentence()), cx),
            });
        })
        .detach();
    }

    /// Attaches the dialog to the terminal of a started sign-in.
    fn show_sign_in_terminal(&mut self, account: &str, terminal: String, cx: &mut Context<Self>) {
        let (Some(service), Some(kind)) = (
            self.service.clone(),
            self.marketplace_accounts
                .sign_in
                .as_ref()
                .map(|sign_in| sign_in.kind.clone()),
        ) else {
            return;
        };
        let Some(account) = self.marketplace.account_number(&kind, account) else {
            return;
        };
        let surface = sign_in_surface(service.clone(), terminal.clone(), cx);
        let (pane, id) = (surface.clone(), terminal.clone());
        let follow = cx.spawn(async move |this, cx| {
            follow_sign_in(this, pane, service, id, cx).await;
        });
        self.set_sign_in_phase(
            Phase::Terminal(SignInTerminal {
                account,
                terminal,
                surface,
                _follow: follow,
                ended: false,
            }),
            cx,
        );
    }

    fn set_sign_in_phase(&mut self, phase: Phase, cx: &mut Context<Self>) {
        if let Some(sign_in) = self.marketplace_accounts.sign_in.as_mut() {
            sign_in.phase = phase;
        }
        cx.notify();
    }

    /// The sign-in terminal ended, or can no longer be attached: the service decides the account now.
    fn sign_in_ended(&mut self, terminal: &str, cx: &mut Context<Self>) {
        if let Some(SignIn {
            phase: Phase::Terminal(view),
            ..
        }) = self.marketplace_accounts.sign_in.as_mut()
            && view.terminal == terminal
        {
            view.ended = true;
        }
        cx.notify();
    }

    /// Cancel and Close: a sign-in still running is ended, and the service then discards its account.
    pub(super) fn close_service_sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(sign_in) = self.marketplace_accounts.sign_in.take() else {
            return;
        };
        if let (Phase::Terminal(view), Some(service)) = (sign_in.phase, self.service.clone())
            && !view.ended
        {
            let terminal = view.terminal;
            cx.spawn(async move |_, _| {
                let _ = service.kill_terminal(&terminal, false).await;
            })
            .detach();
        }
        cx.notify();
    }

    /// Try again after a refused or discarded sign-in: a new sign-in of the same kind.
    pub(super) fn retry_service_sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(kind) = self
            .marketplace_accounts
            .sign_in
            .as_ref()
            .map(|sign_in| sign_in.kind.clone())
        else {
            return;
        };
        self.start_service_sign_in(&kind, cx);
    }
}

/// The pane of a sign-in terminal: keys and resizes go to the service. A sign-in is never restarted from here.
fn sign_in_surface(
    service: ServiceHandle,
    terminal: String,
    cx: &mut App,
) -> Entity<TerminalSurface> {
    cx.new(|cx| {
        let (typing, typed_into) = (service.clone(), terminal.clone());
        let (sizing, sized) = (service, terminal);
        TerminalSurface::new(NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS, cx)
            .on_input(move |bytes, _, _| typing.send_terminal_input(&typed_into, bytes.to_vec()))
            .on_resize(move |cols, rows, _, cx| {
                let (service, terminal) = (sizing.clone(), sized.clone());
                cx.spawn(async move |_| {
                    let _ = service.resize_terminal(&terminal, cols, rows).await;
                })
                .detach();
            })
    })
}

/// Follows the sign-in terminal until it ends. The pane answers the program's queries (decision 0036); a lost link
/// attaches again once it is back, and an attach the service refuses means the terminal is gone.
async fn follow_sign_in(
    this: WeakEntity<Shell>,
    surface: Entity<TerminalSurface>,
    service: ServiceHandle,
    terminal: String,
    cx: &mut AsyncApp,
) {
    let mut link = service.watch_connection();
    'attach: loop {
        if !wait_until_connected(&mut link).await {
            return;
        }
        let params = AttachRole::AnswersQueries.attach_params(&terminal, None);
        let mut stream = match service.attach_terminal(params).await {
            Ok(stream) => stream,
            Err(Refusal::Unavailable { .. }) => {
                if link.changed().await.is_err() {
                    return;
                }
                continue;
            }
            Err(_) => break,
        };
        while let Some(item) = stream.next().await {
            match item {
                TerminalStreamItem::Update(update) => {
                    if let Step::Reattach = apply_update(&surface, &service, &terminal, &update, cx)
                    {
                        continue 'attach;
                    }
                }
                TerminalStreamItem::Ended(
                    StreamEnd::Lost | StreamEnd::Failed(Refusal::Unavailable { .. }),
                ) => continue 'attach,
                TerminalStreamItem::Ended(StreamEnd::Finished | StreamEnd::Failed(_)) => break,
            }
        }
        break;
    }
    let _ = this.update(cx, |shell, cx| shell.sign_in_ended(&terminal, cx));
}

/// The dialog of a sign-in this desktop started on the service, if one is open.
pub(super) fn dialog(shell: &Shell, weak: &WeakEntity<Shell>) -> Option<AnyElement> {
    let sign_in = shell.marketplace_accounts.sign_in.as_ref()?;
    let snapshot = shell.model().marketplace();
    let agent = snapshot
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some(sign_in.kind.as_str()))
        .map_or_else(|| sign_in.kind.clone(), |item| item.name.clone());
    let (stage, surface) = match &sign_in.phase {
        Phase::Starting => (SignInStage::Starting, None),
        Phase::Refused(reason) => (
            SignInStage::Failed {
                reason: reason.clone().into(),
            },
            None,
        ),
        Phase::Terminal(view) => {
            let stage = match terminal_sign_in(snapshot, view.account, view.ended) {
                TerminalSignIn::Running => SignInStage::InTerminal,
                TerminalSignIn::Checking => SignInStage::Checking,
                TerminalSignIn::Kept => SignInStage::Done,
                TerminalSignIn::Discarded => SignInStage::Failed {
                    reason: DISCARDED.into(),
                },
            };
            (stage, Some(view.surface.clone()))
        }
    };
    let (retry, done, cancel) = (weak.clone(), weak.clone(), weak.clone());
    let mut dialog = AccountSignInDialog::new("marketplace-sign-in", agent, stage)
        .on_retry(move |_, cx| {
            let _ = retry.update(cx, |shell, cx| shell.retry_service_sign_in(cx));
        })
        .on_done(move |_, cx| {
            let _ = done.update(cx, |shell, cx| shell.close_service_sign_in(cx));
        })
        .on_cancel(move |_, cx| {
            let _ = cancel.update(cx, |shell, cx| shell.close_service_sign_in(cx));
        });
    if let Some(surface) = surface {
        dialog = dialog.terminal(
            div()
                .debug_selector(|| "marketplace-sign-in-terminal".to_owned())
                .size_full()
                .child(surface),
        );
    }
    Some(dialog.into_any_element())
}
