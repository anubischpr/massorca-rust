//! Element and model tests of an item page's banner: its colours, and what a pill press does.

use super::extension_tests::{catalogue_package, open_listed};
use super::item::banner_fill;
use crate::model::home_start::HOME_START;
use crate::shell::service_tests::click;
use gpui::TestAppContext;
use massorca_protocol::methods::extensions::{Banner, BannerExample, ExtensionItem};
use massorca_ui::components::invocation_banner::BannerFill;
use massorca_ui::tokens::{Color, Palette};

/// A catalogue package with two listed colours and one mention example, "@notes summarise my notes".
fn banner_package() -> ExtensionItem {
    let mut package = catalogue_package();
    package.listing.mention = Some("notes".to_owned());
    package.listing.banner = Some(Banner {
        colors: vec!["#3F9B6E".to_owned(), "#8E8E93".to_owned()],
        examples: vec![BannerExample {
            text: Some("@notes".to_owned()),
            caption: "@notes summarise my notes".to_owned(),
            ..crate::service::fixtures::base_banner_example()
        }],
        ..crate::service::fixtures::base_banner()
    });
    package
}

#[test]
fn the_banner_takes_its_listed_colours_or_else_the_palette_colour_of_its_slot() {
    let palette = Palette::DARK;
    let listed = (
        Color::rgba8(0xD9, 0x77, 0x57, 1.0),
        Some(Color::rgba8(0xB5, 0x53, 0x2F, 1.0)),
    );
    assert_eq!(
        banner_fill(Some(listed), 3, &palette),
        BannerFill::Listed(listed.0, listed.1)
    );
    assert_eq!(
        banner_fill(None, 3, &palette),
        BannerFill::Palette(palette.banner.palette_3),
        "slot 3 is banner.palette.3"
    );
    assert_eq!(
        banner_fill(None, 8, &palette),
        BannerFill::Palette(palette.banner.palette_8),
        "slot 8 is banner.palette.8"
    );
}

#[gpui::test]
fn an_item_with_no_example_prompts_has_no_banner_part(cx: &mut TestAppContext) {
    let (_, cx, _) = open_listed(cx, Vec::new(), vec![catalogue_package()]);
    let shown = cx.debug_bounds("marketplace-part-banner").is_some();
    assert!(!shown, "the page goes straight on from the host line");
}

#[gpui::test]
fn a_banner_pill_opens_a_new_chat_with_its_prompt_in_the_composer(cx: &mut TestAppContext) {
    let (rig, cx, _) = open_listed(cx, Vec::new(), vec![banner_package()]);
    let key = rig.shell.read_with(cx, |shell, _| {
        shell
            .model()
            .selected_workspace()
            .map_or_else(|| HOME_START.to_owned(), str::to_owned)
    });
    click(cx, "invocation-pill-0");
    let composer = rig
        .shell
        .update(cx, |shell, cx| shell.start_composer(&key, cx));
    let draft = composer.read_with(cx, |composer, cx| composer.draft(cx));
    assert_eq!(draft, "@notes summarise my notes");
}
