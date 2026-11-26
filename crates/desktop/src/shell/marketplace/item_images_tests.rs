//! Element tests of the listed images, with a scripted service that answers each `extensions.asset` call.

use super::extension_tests::catalogue_package;
use crate::model::navigation::MarketplacePage;
use crate::service::{Command, ExtensionCommand, ExtensionsRead, Refusal, StoreUpdate};
use crate::shell::service_tests::{Rig, draw, rig};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::extensions::{Banner, ExtensionAsset, ExtensionItem};

/// A catalogue package at `version` that lists a logo and, when given, a banner image.
fn package(version: &str, logo: Option<&str>, banner: Option<&str>) -> ExtensionItem {
    let mut item = catalogue_package();
    item.version = version.to_owned();
    item.listing.logo = logo.map(str::to_owned);
    item.listing.banner = banner.map(|image| Banner {
        image: Some(image.to_owned()),
        ..crate::service::fixtures::base_banner()
    });
    item
}

/// A connected shell that lists these packages in the catalogue, with the Discover page drawn.
fn discover(
    cx: &mut TestAppContext,
    packages: Vec<ExtensionItem>,
) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[]);
    catalogue(&rig, cx, 1, packages);
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Discover, cx)
    });
    draw(cx);
    (rig, cx)
}

/// Sends the catalogue as the service lists it. A higher `number` than the reads before it is the newer read.
fn catalogue(rig: &Rig, cx: &mut VisualTestContext, number: u64, packages: Vec<ExtensionItem>) {
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number,
            installed: Ok(Vec::new()),
            catalogue: Ok(packages),
            history: Default::default(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
}

/// The service's answer for `path`, with `bytes` base64 as the service sends them.
fn asset(path: &str, bytes: &[u8]) -> ExtensionAsset {
    ExtensionAsset {
        path: path.to_owned(),
        size: bytes.len() as u64,
        data: STANDARD.encode(bytes),
        ..crate::service::fixtures::base_extension_asset()
    }
}

/// A one-pixel PNG.
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::new(1, 1)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("a one-pixel PNG encodes");
    bytes
}

#[gpui::test]
fn a_card_shows_its_logo_once_the_service_sends_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = discover(cx, vec![package("1.0.0", Some("logo.png"), None)]);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { params, reply })) => {
            assert_eq!(params.path, "logo.png");
            reply
        }
        _ => panic!("the card asks the service for its logo"),
    };
    let _ = reply.send(Ok(asset("logo.png", &png())));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("card-logo").is_some(),
        "the logo replaces the icon"
    );
}

#[gpui::test]
fn a_failed_logo_leaves_the_icon_and_is_not_asked_for_again(cx: &mut TestAppContext) {
    let (mut rig, cx) = discover(cx, vec![package("1.0.0", Some("logo.png"), None)]);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { reply, .. })) => reply,
        _ => panic!("the card asks the service for its logo"),
    };
    let _ = reply.send(Err(Refusal::unavailable("the service is busy")));
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("card-logo").is_none(), "the icon stays");
    draw(cx);
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "a failed logo is not asked for again"
    );
}

#[gpui::test]
fn a_logo_that_does_not_decode_leaves_the_icon(cx: &mut TestAppContext) {
    let (mut rig, cx) = discover(cx, vec![package("1.0.0", Some("logo.png"), None)]);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { reply, .. })) => reply,
        _ => panic!("the card asks the service for its logo"),
    };
    let _ = reply.send(Ok(asset("logo.png", b"not an image")));
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("card-logo").is_none(), "the icon stays");
}

#[gpui::test]
fn a_logo_is_fetched_once_per_version(cx: &mut TestAppContext) {
    let (mut rig, cx) = discover(cx, vec![package("1.0.0", Some("logo.png"), None)]);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { reply, .. })) => reply,
        _ => panic!("the card asks the service for its logo"),
    };
    let _ = reply.send(Ok(asset("logo.png", &png())));
    cx.run_until_parked();
    draw(cx);
    draw(cx);
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "a loaded logo of the same version is not fetched again"
    );

    catalogue(&rig, cx, 2, vec![package("2.0.0", Some("logo.png"), None)]);
    draw(cx);
    cx.run_until_parked();
    assert!(
        matches!(
            rig.script.next(),
            Some(Command::Extension(ExtensionCommand::Asset { .. }))
        ),
        "a new version fetches its logo again"
    );
}

#[gpui::test]
fn an_item_page_draws_its_banner_image_once_the_service_sends_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    catalogue(
        &rig,
        cx,
        1,
        vec![package("1.0.0", None, Some("banner.png"))],
    );
    let id = rig
        .shell
        .read_with(cx, |shell, _| {
            shell
                .model()
                .marketplace()
                .items
                .iter()
                .find(|item| item.name == "Notes")
                .map(|item| item.id)
        })
        .expect("the package is listed");
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Item(id), cx)
    });
    draw(cx);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { params, reply })) => {
            assert_eq!(params.path, "banner.png");
            reply
        }
        _ => panic!("the item page asks the service for its banner"),
    };
    let _ = reply.send(Ok(asset("banner.png", &png())));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("marketplace-banner-image").is_some(),
        "the banner image is drawn"
    );
}

/// Opens the item page of the catalogue's "Notes" package.
fn open_notes_page(rig: &Rig, cx: &mut VisualTestContext) {
    let id = rig
        .shell
        .read_with(cx, |shell, _| {
            shell
                .model()
                .marketplace()
                .items
                .iter()
                .find(|item| item.name == "Notes")
                .map(|item| item.id)
        })
        .expect("the package is listed");
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Item(id), cx)
    });
}

#[gpui::test]
fn an_item_page_header_draws_its_logo_once_the_service_sends_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    catalogue(&rig, cx, 1, vec![package("1.0.0", Some("logo.png"), None)]);
    open_notes_page(&rig, cx);
    draw(cx);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { params, reply })) => {
            assert_eq!(params.path, "logo.png");
            reply
        }
        _ => panic!("the item page asks the service for its logo"),
    };
    let _ = reply.send(Ok(asset("logo.png", &png())));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("marketplace-header-logo").is_some(),
        "the logo replaces the kind icon in the header"
    );
}

#[gpui::test]
fn a_failed_header_logo_leaves_the_kind_icon(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    catalogue(&rig, cx, 1, vec![package("1.0.0", Some("logo.png"), None)]);
    open_notes_page(&rig, cx);
    draw(cx);
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Asset { reply, .. })) => reply,
        _ => panic!("the item page asks the service for its logo"),
    };
    let _ = reply.send(Err(Refusal::unavailable("the service is busy")));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("marketplace-header-logo").is_none(),
        "no logo is drawn"
    );
    assert!(
        cx.debug_bounds("marketplace-header-icon").is_some(),
        "the kind icon stays"
    );
}
