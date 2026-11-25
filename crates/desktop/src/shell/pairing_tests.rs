//! Element tests of the Pairing page: an offer made and an offer accepted, through a scripted service.
//! Each call the page sends is answered by hand, so no window opens and no service runs.

use super::settings_tests::{Rig, click, draw, paired_host, settings_rig};
use crate::model::devices::RevokeFlow;
use crate::model::navigation::Page;
use crate::model::pairing::{AcceptState, OfferState};
use crate::service::{Command, Refusal, StoreUpdate};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::devices::{DeviceInfo, PairingCreated, Secret};

fn open_pairing(rig: &Rig, cx: &mut VisualTestContext) {
    rig.shell
        .update(cx, |shell, _| shell.model.open_page(Page::Pairing));
    draw(cx);
}

fn created(offer: &str) -> PairingCreated {
    PairingCreated {
        offer: Secret::new(offer),
        expires_at_ms: 1_000,
        scopes: vec!["read".into()],
        addresses: vec!["100.64.0.10:7347".into()],
        server_pin: "sha256:test".into(),
        offer_id: None,
    }
}

#[gpui::test]
fn making_an_offer_asks_the_service_and_shows_what_it_answers(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_pairing(&rig, cx);
    click(cx, "pairing-make-offer");
    let reply = loop {
        match rig.script.next() {
            Some(Command::CreateOffer { params, reply }) => {
                assert!(params.admin, "a paired device gets full access");
                break reply;
            }
            Some(_) => continue,
            None => panic!("the offer was not asked for"),
        }
    };
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.pairing.offer.clone()),
        OfferState::Making,
        "the page says it is making the offer while the service works"
    );
    reply
        .send(Ok(created("offer-one")))
        .expect("the page listens");
    cx.run_until_parked();
    let state = rig
        .shell
        .read_with(cx, |shell, _| shell.pairing.offer.clone());
    assert!(
        matches!(&state, OfferState::Made(view) if view.offer == "offer-one"),
        "the offer the service made is shown: {state:?}"
    );
}

#[gpui::test]
fn a_custom_address_is_sent_with_the_offer_and_empty_means_the_bound_ones(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_pairing(&rig, cx);
    let field = rig
        .shell
        .read_with(cx, |shell, _| shell.pairing_address.clone());
    field.update_in(cx, |field, window, cx| {
        field.set_text(" 10.0.2.2, vps.example.ts.net:443 ", window, cx)
    });
    click(cx, "pairing-make-offer");
    let addresses = loop {
        match rig.script.next() {
            Some(Command::CreateOffer { params, .. }) => break params.addresses,
            Some(_) => continue,
            None => panic!("the offer was not asked for"),
        }
    };
    assert_eq!(addresses, ["10.0.2.2", "vps.example.ts.net:443"]);
    assert!(super::pairing::custom_addresses("  ").is_empty());
}

#[gpui::test]
fn a_refused_offer_shows_the_services_own_sentence(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_pairing(&rig, cx);
    click(cx, "pairing-make-offer");
    let reply = loop {
        match rig.script.next() {
            Some(Command::CreateOffer { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the offer was not asked for"),
        }
    };
    let refusal = Refusal::unavailable("the service is not running");
    let sentence = refusal.sentence();
    reply.send(Err(refusal)).expect("the page listens");
    cx.run_until_parked();
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.pairing.offer.clone()),
        OfferState::Refused(sentence)
    );
}

#[gpui::test]
fn pairing_sends_the_typed_offer_and_clears_the_field_once_paired(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_pairing(&rig, cx);
    let field = rig
        .shell
        .read_with(cx, |shell, _| shell.pairing_offer.clone());
    field.update_in(cx, |field, window, cx| {
        field.set_text("  offer-two  ", window, cx)
    });
    click(cx, "pairing-pair");
    let (offer, name, reply) = loop {
        match rig.script.next() {
            Some(Command::AcceptOffer { offer, name, reply }) => break (offer, name, reply),
            Some(_) => continue,
            None => panic!("the offer was not sent"),
        }
    };
    assert_eq!(offer, "offer-two", "the offer is sent trimmed");
    assert_eq!(name, None, "no name typed: the server's own name is used");
    reply
        .send(Ok(paired_host("vps")))
        .expect("the page listens");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.pairing.accept.clone()),
        AcceptState::Paired("vps".to_owned())
    );
    assert_eq!(field.read_with(cx, |field, _| field.text().to_owned()), "");
}

fn device(id: &str, name: &str) -> DeviceInfo {
    DeviceInfo {
        id: id.into(),
        name: name.into(),
        scopes: vec!["read".into()],
        key_pin: "sha256:test".into(),
        paired_at_ms: 0,
        last_seen_ms: None,
        revoked_at_ms: None,
        current: false,
    }
}

#[gpui::test]
fn opening_the_pairing_page_asks_the_service_for_the_devices_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_pairing(&rig, cx);
    draw(cx);
    cx.run_until_parked();
    let mut asks = 0;
    while let Some(command) = rig.script.next() {
        if matches!(command, Command::ListDevices { .. }) {
            asks += 1;
        }
    }
    assert_eq!(asks, 1, "one request each time the page is shown");
}

#[gpui::test]
fn a_device_is_revoked_only_after_the_service_says_so(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    rig._updates
        .send(StoreUpdate::Devices(Ok(vec![device("d1", "Pixel 8")])))
        .expect("the store listens");
    cx.run_until_parked();
    open_pairing(&rig, cx);
    assert!(
        cx.debug_bounds("pairing-revoke-d1").is_some(),
        "the active device offers Revoke on its row"
    );
    let flow = RevokeFlow {
        device: "d1".into(),
        name: "Pixel 8".into(),
        busy: false,
        error: None,
    };
    rig.shell
        .update(cx, |shell, cx| shell.open_revoke(flow, cx));
    draw(cx);
    rig.shell.update(cx, |shell, cx| shell.confirm_revoke(cx));
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            Some(Command::RevokeDevice { params, reply }) => {
                assert_eq!(params.device, "d1");
                break reply;
            }
            Some(_) => continue,
            None => panic!("the revoke was not sent"),
        }
    };
    let refused = Refusal::unavailable("the device is not paired");
    let sentence = refused.sentence();
    reply.send(Err(refused)).expect("the page listens");
    cx.run_until_parked();
    let flow = rig
        .shell
        .read_with(cx, |shell, _| shell.revoke.clone())
        .expect("a refused revoke keeps its confirm open");
    assert!(!flow.busy, "the confirm is no longer busy");
    assert_eq!(flow.error, Some(sentence), "the service's sentence is kept");

    rig.shell.update(cx, |shell, cx| shell.confirm_revoke(cx));
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            Some(Command::RevokeDevice { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the revoke was not sent again"),
        }
    };
    let mut revoked = device("d1", "Pixel 8");
    revoked.revoked_at_ms = Some(1);
    reply.send(Ok(revoked)).expect("the page listens");
    cx.run_until_parked();
    assert!(
        rig.shell.read_with(cx, |shell, _| shell.revoke.is_none()),
        "a revoke that worked closes the confirm"
    );
}
