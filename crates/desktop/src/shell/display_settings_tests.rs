//! Element tests of the display settings (SET-01): a change the store reports reaches the Status bar and the text
//! preferences, a damaged value keeps its default, and this window's own Status bar choice survives a change to
//! another setting. The scripted shell of app_theme_tests stands in for the service.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::config::ConfigValue;
use massorca_ui::text_preferences::{self, TextPreferences};
use serde_json::{Value, json};

use super::app_theme_tests::{Rig, rig};
use crate::model::status_items::StatusItem;
use crate::service::StoreUpdate;

/// Reports `value` for `key` as a change event carries it, and lets the shell follow it.
fn changed(rig: &Rig, cx: &mut VisualTestContext, key: &str, value: Value) {
    rig.updates
        .send(StoreUpdate::ConfigChanged(ConfigValue {
            owner: "core".into(),
            key: key.into(),
            value,
            ..Default::default()
        }))
        .expect("the store listens");
    cx.run_until_parked();
}

/// The Status bar items this window shows, in bar order.
fn shown_items(rig: &Rig, cx: &mut VisualTestContext) -> Vec<StatusItem> {
    rig.shell.read_with(cx, |shell, _| {
        StatusItem::ALL
            .into_iter()
            .filter(|item| shell.model().status_items().is_shown(*item))
            .collect()
    })
}

#[gpui::test]
fn a_status_bar_setting_shows_only_the_items_it_lists(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);

    changed(
        &rig,
        cx,
        "appearance.status_bar",
        json!(["hosts", "agent_status"]),
    );

    assert_eq!(
        shown_items(&rig, cx),
        vec![StatusItem::Agents, StatusItem::Hosts]
    );
}

#[gpui::test]
fn this_windows_status_bar_choice_survives_a_change_to_another_setting(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.shell.update(cx, |shell, _| {
        shell.model.toggle_status_item(StatusItem::Memory)
    });

    changed(&rig, cx, "terminal.scrollback", json!(500));

    assert!(!shown_items(&rig, cx).contains(&StatusItem::Memory));
    changed(&rig, cx, "appearance.status_bar", json!(["memory"]));
    assert_eq!(shown_items(&rig, cx), vec![StatusItem::Memory]);
    text_preferences::set(TextPreferences::default());
}

#[gpui::test]
fn each_text_setting_reaches_the_text_preferences(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);

    changed(&rig, cx, "appearance.text_size", json!(110));
    changed(&rig, cx, "appearance.font.interface", json!("Inter"));
    changed(&rig, cx, "appearance.font.monospace", json!("Fira Code"));
    changed(&rig, cx, "terminal.font", json!("Cascadia Code"));
    changed(&rig, cx, "terminal.scrollback", json!(5000));

    let applied = text_preferences::current();
    assert_eq!(applied.size_percent, 110);
    assert_eq!(applied.interface_family.as_ref(), "Inter");
    assert_eq!(applied.monospace_family.as_ref(), "Fira Code");
    assert_eq!(applied.terminal_family.as_ref(), "Cascadia Code");
    assert_eq!(applied.scrollback_lines, 5000);
    text_preferences::set(TextPreferences::default());
}

#[gpui::test]
fn a_damaged_setting_keeps_its_default_and_the_others_still_apply(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);

    changed(&rig, cx, "appearance.text_size", json!(999));
    changed(&rig, cx, "appearance.status_bar", json!("hosts"));
    changed(&rig, cx, "terminal.scrollback", json!(5000));

    assert_eq!(text_preferences::current().size_percent, 100);
    assert_eq!(shown_items(&rig, cx), StatusItem::ALL.to_vec());
    assert_eq!(text_preferences::current().scrollback_lines, 5000);
    text_preferences::set(TextPreferences::default());
}
