//! Element tests of the Settings rows backed by one core setting (SET-01): each control shows the value the store
//! holds at start, follows a change event, writes through `config.set`, resets through `config.reset` and shows a
//! refusal. The scripted service of settings_tests answers each call by hand.

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};
use massorca_protocol::methods::config::ConfigValue;
use serde_json::{Value, json};

use super::settings_tests::{Rig, click, draw, settings_rig};
use crate::service::{Command, Refusal, StoreUpdate};

/// Clicks inside the box with this selector, 8 px from its left or right edge: a switch's track sits at the right of
/// its label, a checkbox's box at the left of its label.
fn click_edge(cx: &mut VisualTestContext, selector: &'static str, right: bool) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    let x = if right {
        bounds.right() - px(8.)
    } else {
        bounds.left() + px(8.)
    };
    cx.simulate_click(point(x, bounds.center().y), Modifiers::default());
}

fn click_switch(cx: &mut VisualTestContext, selector: &'static str) {
    click_edge(cx, selector, true);
}

/// A core setting as the service lists it, with its range when it has one.
fn setting(key: &str, value: Value, default: Value, range: Option<(i64, i64)>) -> ConfigValue {
    ConfigValue {
        owner: "core".into(),
        key: key.into(),
        is_default: value == default,
        value,
        default,
        min: range.map(|(min, _)| min),
        max: range.map(|(_, max)| max),
        ..Default::default()
    }
}

/// The settings the store holds at start, as the bridge's resync after connecting gives them.
fn start_with(rig: &Rig, cx: &mut VisualTestContext, values: Vec<ConfigValue>) {
    rig._updates
        .send(StoreUpdate::ConfigResync(values))
        .expect("the store listens");
    cx.run_until_parked();
}

/// A change event from the service, as another client's write sends it.
fn changed(rig: &Rig, cx: &mut VisualTestContext, value: ConfigValue) {
    rig._updates
        .send(StoreUpdate::ConfigChanged(value))
        .expect("the store listens");
    cx.run_until_parked();
}

/// The next `config.set` or `config.reset` the page sent, skipping the page's other reads.
fn next_write(rig: &mut Rig) -> Option<Command> {
    while let Some(command) = rig.script.next() {
        if matches!(
            command,
            Command::SetConfig { .. } | Command::ResetConfig { .. }
        ) {
            return Some(command);
        }
    }
    None
}

/// The key and value of the next `config.set`, which is answered with that value stored.
fn answer_set(rig: &mut Rig, cx: &mut VisualTestContext, default: Value) -> (String, Value) {
    cx.run_until_parked();
    match next_write(rig) {
        Some(Command::SetConfig { params, reply }) => {
            let stored = setting(&params.key, params.value.clone(), default, None);
            reply.send(Ok(stored)).expect("the page listens");
            cx.run_until_parked();
            (params.key, params.value)
        }
        _ => panic!("a config.set call was expected"),
    }
}

fn open_section(cx: &mut VisualTestContext, nav: &'static str) {
    click(cx, nav);
    cx.run_until_parked();
}

#[gpui::test]
fn a_switch_writes_the_opposite_of_the_value_read_at_start_and_follows_a_change(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = settings_rig(cx);
    start_with(
        &rig,
        cx,
        vec![setting(
            "agents.computer_use",
            json!(false),
            json!(false),
            None,
        )],
    );
    open_section(cx, "settings-nav-Agents");

    click_switch(cx, "settings-Agents-1-switch");
    assert_eq!(
        answer_set(&mut rig, cx, json!(false)),
        ("agents.computer_use".to_owned(), json!(true))
    );

    // Another client turns it off again; the switch follows, so the next press turns it on.
    changed(
        &rig,
        cx,
        setting("agents.computer_use", json!(false), json!(false), None),
    );
    click_switch(cx, "settings-Agents-1-switch");
    assert_eq!(
        answer_set(&mut rig, cx, json!(false)).1,
        json!(true),
        "the switch follows the change event"
    );
}

#[gpui::test]
fn a_row_sends_nothing_until_the_service_has_listed_its_setting(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_section(cx, "settings-nav-Agents");

    click_switch(cx, "settings-Agents-1-switch");
    cx.run_until_parked();

    assert!(next_write(&mut rig).is_none(), "nothing is listed yet");
}

#[gpui::test]
fn a_stepper_moves_by_its_step_and_stops_at_the_end_of_the_range(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    start_with(
        &rig,
        cx,
        vec![setting(
            "editor.tab_width",
            json!(4),
            json!(4),
            Some((1, 16)),
        )],
    );
    open_section(cx, "settings-nav-Input");

    click(cx, "icon-button:settings-Input-0-increase");
    assert_eq!(
        answer_set(&mut rig, cx, json!(4)),
        ("editor.tab_width".to_owned(), json!(5))
    );

    changed(
        &rig,
        cx,
        setting("editor.tab_width", json!(16), json!(4), Some((1, 16))),
    );
    click(cx, "icon-button:settings-Input-0-increase");
    cx.run_until_parked();
    assert!(next_write(&mut rig).is_none(), "16 is the top of the range");
    click(cx, "icon-button:settings-Input-0-decrease");
    assert_eq!(answer_set(&mut rig, cx, json!(4)).1, json!(15));
}

#[gpui::test]
fn a_status_bar_checkbox_writes_the_whole_set_without_its_item(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    let all = json!(["agent_status", "memory", "terminals", "ports", "hosts"]);
    start_with(
        &rig,
        cx,
        vec![setting(
            "appearance.status_bar",
            all.clone(),
            all.clone(),
            None,
        )],
    );
    open_section(cx, "settings-nav-Appearance");

    click_edge(cx, "settings-Appearance-9-1", false);

    assert_eq!(
        answer_set(&mut rig, cx, all),
        (
            "appearance.status_bar".to_owned(),
            json!(["agent_status", "terminals", "ports", "hosts"])
        )
    );
}

#[gpui::test]
fn a_changed_setting_offers_a_reset_that_asks_the_service_for_the_default(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    start_with(
        &rig,
        cx,
        vec![setting(
            "agents.computer_use",
            json!(true),
            json!(false),
            None,
        )],
    );
    open_section(cx, "settings-nav-Agents");

    click(cx, "icon-button:settings-Agents-1-reset");
    cx.run_until_parked();
    let reply = match next_write(&mut rig) {
        Some(Command::ResetConfig { params, reply }) => {
            assert_eq!(params.key, "agents.computer_use");
            reply
        }
        _ => panic!("a config.reset call was expected"),
    };
    let default = setting("agents.computer_use", json!(false), json!(false), None);
    reply.send(Ok(default.clone())).expect("the page listens");
    // The bridge passes the stored default to the store, as the change event does.
    changed(&rig, cx, default);
    draw(cx);

    assert!(
        cx.debug_bounds("icon-button:settings-Agents-1-reset")
            .is_none(),
        "a setting at its default offers no reset"
    );
}

#[gpui::test]
fn a_refused_write_shows_the_services_sentence_and_the_next_stored_write_clears_it(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = settings_rig(cx);
    start_with(
        &rig,
        cx,
        vec![setting(
            "agents.computer_use",
            json!(false),
            json!(false),
            None,
        )],
    );
    open_section(cx, "settings-nav-Agents");

    click_switch(cx, "settings-Agents-1-switch");
    cx.run_until_parked();
    match next_write(&mut rig) {
        Some(Command::SetConfig { reply, .. }) => reply
            .send(Err(Refusal::Failed {
                code: "config_invalid_value".into(),
                message: "The value is not allowed.".into(),
            }))
            .expect("the page listens"),
        _ => panic!("a config.set call was expected"),
    }
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.setting_notice.clone());
    assert!(
        notice.is_some_and(|(key, _)| key == "agents.computer_use"),
        "the row keeps the refusal"
    );

    click_switch(cx, "settings-Agents-1-switch");
    answer_set(&mut rig, cx, json!(false));
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.setting_notice.clone()),
        None
    );
}
