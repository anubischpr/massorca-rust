//! The Services section of the Marketplace's Services category (SKL-35): the picked host's hosted services, each with
//! its state.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, div, px};
use massorca_protocol::methods::hosted_services::ServiceList;
use massorca_ui::components::badge::Badge;
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM, TYPE_TITLE};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::MarketplaceFrame;
use super::host_pick::effective_target;
use crate::model::marketplace::{HostTarget, ServiceRow, ServicesState};
use crate::service::Refusal;
use crate::shell::Shell;

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
                column = column.child(service_row(row));
            }
        }
    }
    column.into_any_element()
}

fn service_row(row: &ServiceRow) -> AnyElement {
    let id = row.id.clone();
    let secondary = [row.address.clone(), row.detail.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let actions = div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .child(Badge::text(row.state));
    let mut list_row = ListRow::new(format!("marketplace-service-{id}"), row.name.clone())
        .leading(RowLeading::Icon(IconName::Server))
        .trailing(actions);
    if !secondary.is_empty() {
        list_row = list_row.secondary(secondary);
    }
    div()
        .debug_selector(move || format!("marketplace-service-{id}"))
        .child(list_row)
        .into_any_element()
}
