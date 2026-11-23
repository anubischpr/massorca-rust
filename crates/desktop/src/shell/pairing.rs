//! The Pairing page (D-PR-01, D-PR-02): an offer this computer makes for another device, and a pairing
//! this computer accepts from an offer another host made. The page says what the service or the host
//! answered, and nothing before the answer.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, SharedString, WeakEntity, Window, div, px,
};
use massorca_protocol::methods::devices::{DeviceRevokeParams, PairingCreateParams};
use massorca_ui::assets::FONT_MONO;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::components::settings_row::{
    RowStatus, SettingsRow, SettingsSection as SettingsGroup,
};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{
    LAYOUT_PAGE_SETTINGS, RADIUS_MD, SPACE_1, SPACE_2, SPACE_6, SPACE_8, TYPE_BODY_SM,
};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

use super::Shell;
use crate::model::devices::{DeviceRow, DevicesView, REVOKE_LOSS, RevokeFlow, revoke_title};
use crate::model::navigation::Page;
use crate::model::pairing::{
    self, AcceptState, OFFER_WARNING, OfferState, OfferView, PairingState,
};

/// What the page says when there is no service to ask for an offer (sample data, no bridge).
const NO_SERVICE_OFFER: &str = "The MassOrca service is not connected. No offer was made.";
/// What the page says when there is no service to pair through.
const NO_SERVICE_PAIR: &str = "The MassOrca service is not connected. Nothing was paired.";
/// What the confirm says when there is no service to revoke through.
const NO_SERVICE_REVOKE: &str = "The MassOrca service is not connected. Nothing was revoked.";

/// What the Pairing page draws in one frame: the state, the fields, the paired devices and the Shell.
pub(super) struct PairingFrame {
    pub state: PairingState,
    pub offer_field: Entity<TextField>,
    pub name_field: Entity<TextField>,
    /// The devices the store holds: being read, listed, or refused.
    pub devices: DevicesView,
    pub shell: WeakEntity<Shell>,
}

impl Shell {
    /// "Make offer": asks this computer's service for an offer. The page shows it only once the service answers.
    pub(super) fn make_offer(&mut self, cx: &mut Context<Self>) {
        if !pairing::can_make_offer(&self.pairing.offer) {
            return;
        }
        let Some(service) = self.service.clone() else {
            self.pairing.offer = OfferState::Refused(NO_SERVICE_OFFER.to_owned());
            cx.notify();
            return;
        };
        self.pairing.offer = OfferState::Making;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let params = PairingCreateParams {
                ttl_secs: None,
                // A paired device is the owner with full access; older servers default to read and workspace.
                admin: true,
            };
            let result = service.create_offer(params).await;
            let _ = this.update(cx, |shell, cx| {
                shell.pairing.offer = match result {
                    Ok(created) => OfferState::Made(OfferView::from_created(&created)),
                    Err(refusal) => OfferState::Refused(refusal.sentence()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// "Pair": accepts the offer typed in the field. The host is kept under the typed name, or the server's own.
    pub(super) fn accept_typed_offer(&mut self, cx: &mut Context<Self>) {
        let typed = self.pairing_offer.read(cx).text().to_owned();
        if !pairing::can_pair(&typed, &self.pairing.accept) {
            return;
        }
        let name = pairing::name_or_none(self.pairing_name.read(cx).text());
        let Some(service) = self.service.clone() else {
            self.pairing.accept = AcceptState::Refused(NO_SERVICE_PAIR.to_owned());
            cx.notify();
            return;
        };
        let offer = typed.trim().to_owned();
        self.pairing.accept = AcceptState::Pairing;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.accept_offer(offer, name).await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(record) => {
                        shell.pairing.accept = AcceptState::Paired(record.name);
                        shell.pairing.clear_offer_field = true;
                    }
                    Err(refusal) => shell.pairing.accept = AcceptState::Refused(refusal.sentence()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Clears the offer field once a pairing has taken its offer. Clearing needs the window, so the frame does it.
    pub(super) fn sync_pairing_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.pairing.clear_offer_field) {
            let field = self.pairing_offer.clone();
            field.update(cx, |field, cx| field.set_text("", window, cx));
        }
    }
}

impl Shell {
    /// Asks the service for the paired devices each time the Pairing page is shown, and again after a
    /// reconnect. The list reaches the page through the store, not through this call.
    pub(super) fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        if self.model.navigation().page != Page::Pairing {
            self.devices_asked = false;
            return;
        }
        if self.devices_asked {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.devices_asked = true;
        cx.spawn(async move |_, _| {
            let _ = service.list_devices().await;
        })
        .detach();
    }

    /// "Revoke" on a device row: the confirm opens first, and the device stays listed until the service answers.
    pub(super) fn open_revoke(&mut self, flow: RevokeFlow, cx: &mut Context<Self>) {
        self.revoke = Some(flow);
        cx.notify();
    }

    /// Cancel or Escape: closes the confirm unless a revoke is being sent.
    pub(super) fn close_revoke(&mut self, cx: &mut Context<Self>) {
        if self.revoke.as_ref().is_some_and(|flow| !flow.busy) {
            self.revoke = None;
            cx.notify();
        }
    }

    /// "Revoke" in the confirm: sends the revoke. The list changes only when the service says it did.
    pub(super) fn confirm_revoke(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(flow) = self.revoke.as_mut() else {
            return;
        };
        if flow.busy {
            return;
        }
        let device = flow.device.clone();
        flow.error = None;
        let Some(service) = service else {
            flow.error = Some(NO_SERVICE_REVOKE.to_owned());
            cx.notify();
            return;
        };
        flow.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service.revoke_device(DeviceRevokeParams { device }).await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(_) => shell.revoke = None,
                    Err(refusal) => {
                        if let Some(flow) = shell.revoke.as_mut() {
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

/// The Pairing page: this computer's offer, then the pairing it accepts, each a settings group.
pub(super) fn pairing_page(frame: &PairingFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let typed = frame.offer_field.read(cx).text().to_owned();
    div()
        .id("pairing-page")
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .bg(theme.palette.surface.canvas)
        .child(
            div()
                .max_w(px(LAYOUT_PAGE_SETTINGS))
                .w_full()
                .p(px(SPACE_6))
                .flex()
                .flex_col()
                .gap(px(SPACE_8))
                .child(offer_group(frame, &theme))
                .child(accept_group(frame, &typed))
                .child(devices_group(frame, &theme)),
        )
        .into_any_element()
}

/// This computer's offer: the button, what the service answered, and the offer once it is made.
fn offer_group(frame: &PairingFrame, theme: &Theme) -> AnyElement {
    let now = pairing::now_ms();
    let (helper, label, can_make) = match &frame.state.offer {
        OfferState::Idle => (
            "Makes an offer that another device can use once.".to_owned(),
            "Make offer",
            true,
        ),
        OfferState::Making => ("Making an offer.".to_owned(), "Making offer", false),
        OfferState::Made(view) => (
            pairing::expiry_text(view.expires_at_ms, now),
            "Make a new offer",
            true,
        ),
        OfferState::Refused(_) => ("No offer was made.".to_owned(), "Make offer", true),
    };
    let shell = frame.shell.clone();
    let button = Button::new("pairing-make-offer", label)
        .variant(ButtonVariant::Primary)
        .disabled(!can_make)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.make_offer(cx));
        });
    let button = div()
        .debug_selector(|| "pairing-make-offer".to_owned())
        .child(button);
    let mut make = SettingsRow::new("pairing-offer-make", "Offer").helper(helper);
    if let OfferState::Refused(sentence) = &frame.state.offer {
        make = make.status(RowStatus::Error(sentence.clone().into()));
    }
    let mut group = SettingsGroup::new("pairing-offer", "Pair a device")
        .description(
            "Make an offer and enter it on the other device. An offer works once and expires.",
        )
        .row(make.control(button));
    if let OfferState::Made(view) = &frame.state.offer {
        group = group.row(offer_text_row(view, theme));
    }
    group.into_any_element()
}

/// The offer as text, in mono, with a copy button. It is shown only here, while it is made.
fn offer_text_row(view: &OfferView, theme: &Theme) -> AnyElement {
    let text = div()
        .flex_1()
        .min_w_0()
        .px(px(SPACE_2))
        .py(px(SPACE_1))
        .rounded(px(RADIUS_MD))
        .bg(theme.palette.surface.sunken)
        .font_family(FONT_MONO)
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(view.offer.clone());
    let offer = view.offer.clone();
    let copy = Button::new("pairing-copy-offer", "Copy offer")
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(offer.clone())));
    SettingsRow::new("pairing-offer-text", "Offer code")
        .helper(OFFER_WARNING)
        .control(div().flex().gap(px(SPACE_2)).child(text).child(copy))
        .into_any_element()
}

/// Accepting an offer another host made: the offer, an optional name, and the Pair button with its result.
fn accept_group(frame: &PairingFrame, typed: &str) -> AnyElement {
    let (helper, refusal) = match &frame.state.accept {
        AcceptState::Idle => (
            "Keeps the host in Settings and connects to it.".to_owned(),
            None,
        ),
        AcceptState::Pairing => ("Pairing with the host.".to_owned(), None),
        AcceptState::Paired(name) => (format!("Paired with {name}."), None),
        AcceptState::Refused(sentence) => (
            "Keeps the host in Settings and connects to it.".to_owned(),
            Some(sentence.clone()),
        ),
    };
    let shell = frame.shell.clone();
    let can_pair = pairing::can_pair(typed, &frame.state.accept);
    let pair = Button::new("pairing-pair", "Pair")
        .variant(ButtonVariant::Primary)
        .disabled(!can_pair)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.accept_typed_offer(cx));
        });
    let pair = div()
        .debug_selector(|| "pairing-pair".to_owned())
        .child(pair);
    let mut pair_row = SettingsRow::new("pairing-accept-pair", "Pair with the host").helper(helper);
    if let Some(sentence) = refusal {
        pair_row = pair_row.status(RowStatus::Error(sentence.into()));
    }
    SettingsGroup::new("pairing-accept", "Pair with another host")
        .description("Paste the offer that another host shows. This computer keeps that host and connects to it.")
        .row(
            SettingsRow::new("pairing-accept-offer", "Offer")
                .helper("The offer that another host shows.")
                .control(frame.offer_field.clone()),
        )
        .row(
            SettingsRow::new("pairing-accept-name", "Name")
                .helper("Leave empty to use the server's name.")
                .control(frame.name_field.clone()),
        )
        .row(pair_row.control(pair))
        .into_any_element()
}

/// The paired devices: what the service listed, or why it could not. Each device shows its state, and an
/// active device that is not this computer offers Revoke.
fn devices_group(frame: &PairingFrame, theme: &Theme) -> AnyElement {
    let group = SettingsGroup::new("pairing-devices", "Paired devices")
        .description("Devices that can connect to this PC. Revoke one to take its access away.");
    let group = match &frame.devices {
        DevicesView::Reading => group.row(
            SettingsRow::new("pairing-devices-reading", "Devices")
                .helper("Reading the paired devices."),
        ),
        DevicesView::Refused(sentence) => group.row(
            SettingsRow::new("pairing-devices-refused", "Devices")
                .helper("The list is not available.")
                .status(RowStatus::Error(sentence.clone().into())),
        ),
        DevicesView::Listed(rows) if rows.is_empty() => group.row(
            SettingsRow::new("pairing-devices-none", "No paired devices")
                .helper("Make an offer above, and enter it on the other device."),
        ),
        DevicesView::Listed(rows) => rows
            .iter()
            .fold(group, |group, row| group.row(device_row(row, frame, theme))),
    };
    group.into_any_element()
}

/// One paired device: its name, when it was paired and last seen, a label when its access is limited, its state and, when it can be revoked, Revoke.
fn device_row(row: &DeviceRow, frame: &PairingFrame, theme: &Theme) -> AnyElement {
    let state = div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(row.state.word());
    let mut control = div().flex().items_center().gap(px(SPACE_2));
    if let Some(limited) = &row.limited {
        control = control.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(limited.clone()),
        );
    }
    control = control.child(state);
    if row.revocable {
        let flow = RevokeFlow::new(row);
        let shell = frame.shell.clone();
        let selector = format!("pairing-revoke-{}", row.id);
        let id = SharedString::from(format!("pairing-revoke-button-{}", row.id));
        let button = Button::new(id, "Revoke")
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_revoke(flow.clone(), cx));
            });
        control = control.child(div().debug_selector(move || selector.clone()).child(button));
    }
    SettingsRow::new(
        SharedString::from(format!("pairing-device-{}", row.id)),
        row.name.clone(),
    )
    .helper(row.paired.clone())
    .control(control)
    .into_any_element()
}

/// The confirm before a device is revoked: its name in the title, what is lost, and the service's sentence
/// when it refused. Revoke is the danger action, and Cancel starts with the focus.
pub(super) fn revoke_dialog(flow: &RevokeFlow, shell: &WeakEntity<Shell>) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let dialog = Dialog::new("revoke-device", revoke_title(&flow.name))
        .variant(DialogVariant::Destructive)
        .open(true)
        .busy(flow.busy)
        .loss([REVOKE_LOSS])
        .action("Revoke")
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_revoke(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_revoke(cx));
        });
    match &flow.error {
        Some(error) => dialog.error(error.clone()),
        None => dialog,
    }
}
