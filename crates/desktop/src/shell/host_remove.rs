//! Remove host (D-HO-03, merged into X-DLG-01, artboard `sh-confirm-destructive`). The confirm names the host, says
//! nothing on the server is deleted and counts what leaves this app. "Remove host" calls the same client functions as
//! `massorca host remove`: this device is revoked on the host when it answers, then the record is deleted here.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, SharedString, WeakEntity, div, px};
use massorca_client::DeviceRevocation;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::{SPACE_2, SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use crate::service::{HostLoss, HostRemoval};

const NO_SERVICE: &str = "This window has no MassOrca service link. Nothing was removed.";

/// What the confirm knows of the host's projects and sessions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LossCount {
    Counting,
    Counted(HostLoss),
    /// The host did not answer the count, or refused it.
    Uncounted,
}

/// The remove confirm on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostRemoveFlow {
    pub(super) name: String,
    pub(super) loss: LossCount,
    pub(super) busy: bool,
    pub(super) error: Option<String>,
}

pub(super) fn remove_title(name: &str) -> String {
    format!("Remove host {name}?")
}

pub(super) fn remove_body(name: &str) -> String {
    format!(
        "MassOrca stops showing {name} and the projects on it in this app. Nothing on the server is deleted."
    )
}

/// The loss list: counts once the host answered, and what is known when it did not.
pub(super) fn loss_rows(name: &str, loss: &LossCount) -> Vec<String> {
    match loss {
        LossCount::Counting => vec![format!("Counting the projects and sessions on {name}")],
        LossCount::Counted(HostLoss { projects, sessions }) => vec![
            match projects {
                1 => "1 project leaves the sidebar".to_owned(),
                n => format!("{n} projects leave the sidebar"),
            },
            match sessions {
                1 => format!("1 past session on {name} is hidden here"),
                n => format!("{n} past sessions on {name} are hidden here"),
            },
            format!("This PC loses access to {name} until it pairs again"),
        ],
        LossCount::Uncounted => vec![
            format!("The projects and sessions on {name} could not be counted"),
            format!("This PC stays paired on {name} if it does not answer"),
        ],
    }
}

/// The line left after a removal, when something is left behind: the device still paired, or files not deleted yet.
pub(super) fn removal_notice(removal: &HostRemoval) -> Option<String> {
    let host = &removal.host;
    let mut parts = Vec::new();
    if let DeviceRevocation::NotRevoked(reason) = &removal.device {
        parts.push(format!(
            "{host} was removed, but this PC stays paired on it: {reason}. Revoke this PC on {host}'s Pairing page."
        ));
    }
    if let Some(leftover) = &removal.leftover {
        if parts.is_empty() {
            parts.push(format!("{host} was removed."));
        }
        parts.push(format!(
            "Its old files were moved to {}; the next host change deletes them.",
            leftover.display()
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

impl Shell {
    /// Opens the confirm for host `name` and asks the host what it holds, for the counts.
    pub(super) fn open_remove_host(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.host_remove = Some(HostRemoveFlow {
                name,
                loss: LossCount::Uncounted,
                busy: false,
                error: Some(NO_SERVICE.to_owned()),
            });
            cx.notify();
            return;
        };
        self.host_remove = Some(HostRemoveFlow {
            name: name.clone(),
            loss: LossCount::Counting,
            busy: false,
            error: None,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let counted = service.count_host_loss(name.clone()).await;
            let _ = this.update(cx, |shell, cx| {
                if let Some(flow) = shell.host_remove.as_mut().filter(|flow| flow.name == name) {
                    flow.loss = counted.map_or(LossCount::Uncounted, LossCount::Counted);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Cancel or Escape: closes the confirm unless a removal is running.
    pub(super) fn close_remove_host(&mut self, cx: &mut Context<Self>) {
        if self.host_remove.as_ref().is_some_and(|flow| !flow.busy) {
            self.host_remove = None;
            cx.notify();
        }
    }

    /// "Remove host": removes it; the confirm closes once the record is gone, and stays open with the reason if not.
    pub(super) fn confirm_remove_host(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(flow) = self.host_remove.as_mut().filter(|flow| !flow.busy) else {
            return;
        };
        flow.busy = true;
        flow.error = None;
        let name = flow.name.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.remove_host(name).await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(removal) => {
                        shell.host_remove = None;
                        shell.host_remove_notice = removal_notice(&removal);
                    }
                    Err(refusal) => {
                        if let Some(flow) = shell.host_remove.as_mut() {
                            flow.busy = false;
                            flow.error = Some(refusal.sentence());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// The "Remove" button of a paired host's row; it opens the confirm.
pub(super) fn remove_button(name: &str, shell: &WeakEntity<Shell>) -> AnyElement {
    let shell = shell.clone();
    let selector = format!("settings-host-remove-{name}");
    let owned = name.to_owned();
    let button = Button::new(
        SharedString::from(format!("settings-host-remove-button-{name}")),
        "Remove",
    )
    .variant(ButtonVariant::Ghost)
    .on_click(move |_, cx| {
        let name = owned.clone();
        let _ = shell.update(cx, |shell, cx| shell.open_remove_host(name, cx));
    });
    div()
        .debug_selector(move || selector.clone())
        .child(button)
        .into_any_element()
}

/// The destructive confirm: Cancel has the focus, "Remove host" is the danger action.
pub(super) fn remove_host_dialog(
    flow: &HostRemoveFlow,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let body = div()
        .debug_selector(|| "host-remove-body".to_owned())
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(remove_body(&flow.name));
    let dialog = Dialog::new("host-remove", remove_title(&flow.name))
        .variant(DialogVariant::Destructive)
        .open(true)
        .child(body)
        .loss(loss_rows(&flow.name, &flow.loss))
        .action("Remove host")
        .busy(flow.busy)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_remove_host(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_remove_host(cx));
        });
    match &flow.error {
        Some(error) => dialog.error(error.clone()),
        None => dialog,
    }
}

/// What a removal left behind, until dismissed.
pub(super) fn removal_banner(notice: &str, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    let banner = Banner::new(
        "host-remove-notice",
        BannerVariant::Warning,
        notice.to_owned(),
    )
    .on_dismiss(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.host_remove_notice = None;
            cx.notify();
        });
    });
    div().px(px(SPACE_3)).pt(px(SPACE_2)).child(banner)
}
