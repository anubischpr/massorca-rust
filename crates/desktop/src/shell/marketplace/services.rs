//! The Services section of the Marketplace's Services category (SKL-35): the picked host's hosted services, each with
//! its state, and the connect dialog. The token is typed into a masked field and cleared the moment it is sent;
//! nothing in the shell keeps it.

use std::fmt;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, Subscription, WeakEntity, Window, div, px};
use massorca_protocol::methods::hosted_services::{ServiceConnect, ServiceList, ServiceStatus};
use massorca_ui::components::badge::Badge;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM, TYPE_TITLE};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::MarketplaceFrame;
use super::host_pick::effective_target;
use crate::model::marketplace::{
    HostTarget, ServiceAction, ServiceRow, ServicesState, connect_fields,
};
use crate::service::Refusal;
use crate::shell::Shell;

/// The connect dialog of one service. Its fields are cleared when it sends or closes.
pub(in crate::shell) struct ServiceDialog {
    service: String,
    name: String,
    token: Entity<TextField>,
    base_url: Option<Entity<TextField>>,
    email: Option<Entity<TextField>>,
    sending: bool,
    error: Option<String>,
    _changes: Vec<Subscription>,
}

impl ServiceDialog {
    #[cfg(test)]
    pub(super) fn token_field(&self) -> Entity<TextField> {
        self.token.clone()
    }
}

impl fmt::Debug for ServiceDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceDialog")
            .field("service", &self.service)
            .field("sending", &self.sending)
            .finish_non_exhaustive()
    }
}

/// The text of an optional field, trimmed; `None` when it is empty.
fn optional(field: Option<&Entity<TextField>>, cx: &App) -> Option<String> {
    field
        .map(|field| field.read(cx).text().trim().to_owned())
        .filter(|text| !text.is_empty())
}

impl Shell {
    /// The paired host the Services section reads, or `None` for this computer.
    fn services_host(&self) -> Option<String> {
        match effective_target(self.marketplace_pick.target(), &self.marketplace_offers) {
            HostTarget::Paired(name) => Some(name),
            HostTarget::Local => None,
        }
    }

    /// Reads the picked host's services under a new number, so an older answer is dropped.
    pub(super) fn read_hosted_services(&mut self, cx: &mut Context<Self>) {
        let host = self.services_host();
        let request = self.marketplace_services.begin(host.clone());
        self.marketplace_service_error = None;
        let Some(service) = self.service.clone() else {
            let _ = self.marketplace_services.accept(
                request,
                Err(Refusal::unavailable("The service is not connected.")),
            );
            return;
        };
        cx.spawn(async move |this, cx| {
            let result: Result<ServiceList, Refusal> = service.hosted_services(host).await;
            let _ = this.update(cx, |shell, cx| {
                if shell.marketplace_services.accept(request, result) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn open_service_dialog(&mut self, row: &ServiceRow, cx: &mut Context<Self>) {
        let fields = connect_fields(&row.id);
        let token = cx.new(|cx| TextField::new(cx).label("Token").masked().mono());
        let base_url = fields
            .base_url
            .map(|label| cx.new(|cx| TextField::new(cx).label(label).placeholder("https://")));
        let email = fields.email.then(|| {
            cx.new(|cx| TextField::new(cx).label("Atlassian email (only with an API token)"))
        });
        let mut changes = vec![cx.observe(&token, |_, _, cx| cx.notify())];
        changes.extend(
            [&base_url, &email]
                .into_iter()
                .flatten()
                .map(|field| cx.observe(field, |_, _, cx| cx.notify())),
        );
        self.marketplace_service_dialog = Some(ServiceDialog {
            service: row.id.clone(),
            name: row.name.clone(),
            token,
            base_url,
            email,
            sending: false,
            error: None,
            _changes: changes,
        });
        cx.notify();
    }

    /// Sends the token once and clears its field at once; a refusal asks for it again.
    fn connect_service(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (host, request) = (self.services_host(), self.marketplace_services.request());
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(dialog) = self.marketplace_service_dialog.as_mut() else {
            return;
        };
        let token = dialog.token.read(cx).text().trim().to_owned();
        if dialog.sending || token.is_empty() {
            return;
        }
        let params = ServiceConnect {
            service: dialog.service.clone(),
            token,
            base_url: optional(dialog.base_url.as_ref(), cx),
            email: optional(dialog.email.as_ref(), cx),
        };
        dialog
            .token
            .update(cx, |field, cx| field.clear_text(window, cx));
        dialog.sending = true;
        dialog.error = None;
        cx.spawn(async move |this, cx| {
            let result = service.connect_hosted_service(host, params).await;
            let _ = this.update(cx, |shell, cx| shell.service_connected(request, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn service_connected(
        &mut self,
        request: u64,
        result: Result<ServiceStatus, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(status) => {
                self.marketplace_services.update_row(request, &status);
                self.marketplace_service_dialog = None;
            }
            Err(refusal) => {
                if let Some(dialog) = self.marketplace_service_dialog.as_mut() {
                    dialog.sending = false;
                    dialog.error = Some(refusal.sentence());
                }
            }
        }
        cx.notify();
    }

    fn close_service_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = self.marketplace_service_dialog.take() {
            dialog
                .token
                .update(cx, |field, cx| field.clear_text(window, cx));
        }
        cx.notify();
    }

    /// Disconnects a service; a refusal, such as GitLab's, is shown under the section.
    pub(super) fn disconnect_service(&mut self, id: String, cx: &mut Context<Self>) {
        let (host, request) = (self.services_host(), self.marketplace_services.request());
        let Some(service) = self.service.clone() else {
            return;
        };
        self.marketplace_service_error = None;
        cx.spawn(async move |this, cx| {
            let result = service.disconnect_hosted_service(host, id.clone()).await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(status) => shell.marketplace_services.update_row(request, &status),
                    Err(refusal) => {
                        shell.marketplace_service_error = Some((id, refusal.sentence()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// The Services section: a heading, then one row per service with its state and actions.
pub(super) fn section(frame: &MarketplaceFrame, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let mut column = div().flex().flex_col().gap(px(SPACE_2)).child(
        div()
            .text_style(TYPE_TITLE)
            .text_color(p.text.primary)
            .child("Hosted services"),
    );
    match &frame.services.state {
        ServicesState::Loading => {
            column = column.child(ListRow::new("marketplace-services-loading", "").loading(true));
        }
        ServicesState::Failed(sentence) => {
            column = column.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(sentence.clone()),
            );
        }
        ServicesState::Ready(rows) => {
            for row in rows {
                let error = frame
                    .service_error
                    .as_ref()
                    .filter(|(id, _)| *id == row.id)
                    .map(|(_, sentence)| sentence.clone());
                column = column.child(service_row(frame, row, error));
            }
        }
    }
    column.into_any_element()
}

fn service_row(frame: &MarketplaceFrame, row: &ServiceRow, error: Option<String>) -> AnyElement {
    let id = row.id.clone();
    let secondary = [row.address.clone(), row.detail.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let mut actions = div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .child(Badge::text(row.state));
    let label = match row.action {
        ServiceAction::Connect => Some("Connect"),
        ServiceAction::Replace => Some("Replace token"),
        ServiceAction::None => None,
    };
    if let Some(label) = label {
        let (shell, chosen) = (frame.shell.clone(), row.clone());
        actions = actions.child(
            Button::new(format!("marketplace-service-connect-{id}"), label)
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.open_service_dialog(&chosen, cx));
                }),
        );
    }
    if row.can_disconnect {
        let (shell, id) = (frame.shell.clone(), id.clone());
        actions = actions.child(
            Button::new(format!("marketplace-service-disconnect-{id}"), "Disconnect")
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.disconnect_service(id.clone(), cx));
                }),
        );
    }
    let mut list_row = ListRow::new(format!("marketplace-service-{id}"), row.name.clone())
        .leading(RowLeading::Icon(IconName::Server))
        .trailing(actions);
    if !secondary.is_empty() {
        list_row = list_row.secondary(secondary);
    }
    if let Some(error) = error {
        list_row = list_row.error(error);
    }
    div()
        .debug_selector(move || format!("marketplace-service-{id}"))
        .child(list_row)
        .into_any_element()
}

/// The connect dialog, when one is open.
pub(super) fn dialog(shell: &Shell, weak: &WeakEntity<Shell>, cx: &App) -> Option<AnyElement> {
    let dialog = shell.marketplace_service_dialog.as_ref()?;
    let (send, dismiss) = (weak.clone(), weak.clone());
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(dialog.token.clone());
    for field in [&dialog.base_url, &dialog.email].into_iter().flatten() {
        body = body.child(field.clone());
    }
    let empty = dialog.token.read(cx).text().trim().is_empty();
    let mut element = Dialog::new(
        "marketplace-service-dialog",
        format!("Connect {}", dialog.name),
    )
    .open(true)
    .child(body)
    .action("Connect")
    .cancel_label("Cancel")
    .busy(dialog.sending)
    .action_disabled(empty || dialog.sending)
    .on_action(move |window, cx| {
        let _ = send.update(cx, |shell, cx| shell.connect_service(window, cx));
    })
    .on_dismiss(move |window, cx| {
        let _ = dismiss.update(cx, |shell, cx| shell.close_service_dialog(window, cx));
    });
    if let Some(error) = &dialog.error {
        element = element.error(error.clone());
    }
    Some(element.into_any_element())
}
